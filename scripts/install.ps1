[CmdletBinding()]
param(
    [ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version = '0.1.6',
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Nagi\versions'),
    [switch]$NoPath
)
$ErrorActionPreference = 'Stop'
$architecture = $env:PROCESSOR_ARCHITEW6432
if (-not $architecture) { $architecture = $env:PROCESSOR_ARCHITECTURE }
if ($env:OS -ne 'Windows_NT' -or $architecture -ne 'AMD64') {
    throw 'This installer supports Windows x64. Use install.sh on Linux/macOS.'
}
$stem = "nagi-$Version-windows-x86_64"
$asset = "$stem.zip"
$base = "https://github.com/disnana/Nagi/releases/download/nagi-v$Version"
$InstallDir = [IO.Path]::GetFullPath($InstallDir)
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
$work = Join-Path $InstallDir ('.install.' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
$previousTls = [Net.ServicePointManager]::SecurityProtocol
try {
    [Net.ServicePointManager]::SecurityProtocol = $previousTls -bor [Net.SecurityProtocolType]::Tls12
    foreach ($file in @($asset, "$asset.sha256")) {
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$file" -OutFile (Join-Path $work $file)
    }
    $checksum = (Get-Content -Raw (Join-Path $work "$asset.sha256")).Trim()
    if ($checksum -notmatch '^([0-9a-f]{64})  (\S+)$' -or $Matches[2] -ne $asset) { throw 'Invalid checksum file' }
    $expected = $Matches[1]
    $archive = Join-Path $work $asset
    if ((Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) { throw 'SHA-256 mismatch; nothing installed' }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        foreach ($entry in $zip.Entries) {
            $name = $entry.FullName.Replace('\', '/')
            $kind = ($entry.ExternalAttributes -shr 16) -band 0xf000
            if (-not $name.StartsWith("$stem/", [StringComparison]::Ordinal) -or
                ($name.Split('/') | Where-Object { $_ -eq '..' -or $_ -eq '.' -or $_.Contains(':') }) -or
                $kind -notin @(0, 0x8000, 0x4000)) { throw 'Unsafe archive entry' }
        }
    } finally { $zip.Dispose() }
    Expand-Archive -LiteralPath $archive -DestinationPath $work
    $root = Join-Path $work $stem
    foreach ($file in @('runtime\Cargo.toml', 'runtime\src\lib.rs', 'release.json', 'nagic.exe')) {
        if (-not (Test-Path -LiteralPath (Join-Path $root $file) -PathType Leaf)) { throw 'Incomplete distribution' }
    }
    $metadata = Get-Content -Raw (Join-Path $root 'release.json') | ConvertFrom-Json
    if ($metadata.version -ne $Version -or $metadata.platform -ne 'windows-x86_64') { throw 'Distribution metadata mismatch' }
    $actual = & (Join-Path $root 'nagic.exe') --version
    if ($LASTEXITCODE -ne 0 -or $actual -ne "nagic $Version") { throw 'Compiler version mismatch' }
    $destination = Join-Path $InstallDir $stem
    if (Test-Path -LiteralPath $destination) {
        foreach ($file in Get-ChildItem -LiteralPath $root -Recurse -File) {
            $relative = $file.FullName.Substring($root.Length + 1)
            $existing = Join-Path $destination $relative
            if (-not (Test-Path -LiteralPath $existing -PathType Leaf) -or
                (Get-FileHash -LiteralPath $file.FullName).Hash -ne (Get-FileHash -LiteralPath $existing).Hash) {
                throw "Existing install differs: $destination (not overwritten)"
            }
        }
    } else { Move-Item -LiteralPath $root -Destination $destination }
    if (-not $NoPath) {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $parts = @($userPath -split ';' | Where-Object { $_ -and $_.TrimEnd('\') -ine $destination })
        [Environment]::SetEnvironmentVariable('Path', (@($destination) + $parts -join ';'), 'User')
    }
    $env:Path = $destination + ';' + $env:Path
    & (Join-Path $destination 'nagic.exe') --version
    Write-Host "Installed: $destination"
    Write-Host 'Building applications requires Rust/Cargo and Visual Studio C++ Build Tools. Restart VS Code to refresh PATH.'
    if ($env:NAGI_ROOT) { Write-Warning 'NAGI_ROOT is set; remove it to use the installed runtime automatically.' }
} finally {
    [Net.ServicePointManager]::SecurityProtocol = $previousTls
    Remove-Item -LiteralPath $work -Recurse -Force
}
