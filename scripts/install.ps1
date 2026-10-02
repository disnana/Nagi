[CmdletBinding()]
param(
    [ValidatePattern('^(latest|\d+\.\d+\.\d+)$')][string]$Version = 'latest',
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Nagi\versions'),
    [switch]$NoPath
)
$ErrorActionPreference = 'Stop'
$architecture = $env:PROCESSOR_ARCHITEW6432
if (-not $architecture) { $architecture = $env:PROCESSOR_ARCHITECTURE }
if ($env:OS -ne 'Windows_NT' -or $architecture -ne 'AMD64') {
    throw 'This installer supports Windows x64. Use install.sh on Linux/macOS.'
}
$InstallDir = [IO.Path]::GetFullPath($InstallDir).TrimEnd('\', '/')
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
$InstallDir = (Get-Item -LiteralPath $InstallDir).FullName
$lock = $null
$work = Join-Path $InstallDir ('.install.' + [guid]::NewGuid())
$current = Join-Path $InstallDir 'current'
$backup = Join-Path $work 'previous-current'
$next = Join-Path $work 'next-current'
$previousTls = [Net.ServicePointManager]::SecurityProtocol
$previousPath = $env:Path
$previousUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$activated = $false
$committed = $false
$pathChanged = $false
$createdDestination = $false

function Write-InstallStatus([string]$Message, [ConsoleColor]$Color) {
    if (-not $env:NO_COLOR -and $env:TERM -ne 'dumb' -and -not [Console]::IsOutputRedirected) {
        Write-Host $Message -ForegroundColor $Color
    } else { Write-Host $Message }
}

function Get-Distribution([string]$Release, [string]$Folder) {
    $name = "nagi-$Release-windows-x86_64"
    $asset = "$name.zip"
    $base = "https://github.com/disnana/Nagi/releases/download/nagi-v$Release"
    New-Item -ItemType Directory -Path $Folder | Out-Null
    foreach ($file in @($asset, "$asset.sha256")) {
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$file" -OutFile (Join-Path $Folder $file)
    }
    $checksum = (Get-Content -Raw -LiteralPath (Join-Path $Folder "$asset.sha256")).Trim()
    if ($checksum -notmatch '^([0-9a-f]{64})  (\S+)$' -or $Matches[2] -ne $asset) { throw 'Invalid checksum file' }
    $expected = $Matches[1]
    $archive = Join-Path $Folder $asset
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) { throw 'SHA-256 mismatch; nothing installed' }
    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        foreach ($entry in $zip.Entries) {
            $path = $entry.FullName.Replace('\', '/')
            $kind = ($entry.ExternalAttributes -shr 16) -band 0xf000
            if (-not $path.StartsWith("$name/", [StringComparison]::Ordinal) -or
                $path -notmatch '^[a-zA-Z0-9._/-]+$' -or
                ($path.Split('/') | Where-Object { $_ -eq '..' -or $_ -eq '.' }) -or
                $kind -notin @(0, 0x8000, 0x4000)) { throw 'Unsafe archive entry' }
        }
    } finally { $zip.Dispose() }
    Expand-Archive -LiteralPath $archive -DestinationPath $Folder
    $root = Join-Path $Folder $name
    foreach ($file in @('runtime\Cargo.toml', 'runtime\src\lib.rs', 'release.json', 'nagic.exe', 'LICENSE', 'README.txt')) {
        if (-not (Test-Path -LiteralPath (Join-Path $root $file) -PathType Leaf)) { throw 'Incomplete distribution' }
    }
    $metadata = Get-Content -Raw -LiteralPath (Join-Path $root 'release.json') | ConvertFrom-Json
    if ($metadata.version -ne $Release -or $metadata.platform -ne 'windows-x86_64') { throw 'Distribution metadata mismatch' }
    return $root
}

function Get-InstallSnapshot([string]$Root) {
    $item = Get-Item -Force -LiteralPath $Root
    if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Not a plain installation directory' }
    $pending = New-Object 'System.Collections.Generic.Queue[string]'
    $entries = New-Object 'System.Collections.Generic.List[string]'
    $pending.Enqueue($Root)
    while ($pending.Count) {
        foreach ($child in Get-ChildItem -Force -LiteralPath $pending.Dequeue()) {
            $relative = $child.FullName.Substring($Root.Length + 1).Replace('\', '/')
            if ($relative -notmatch '^[a-zA-Z0-9._/-]+$' -or ($child.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Installation contains a link or an unrecognized file name' }
            if ($child.PSIsContainer) {
                $entries.Add("D $relative")
                $pending.Enqueue($child.FullName)
            } else {
                $digest = (Get-FileHash -LiteralPath $child.FullName -Algorithm SHA256).Hash
                $entries.Add("F $digest $relative")
            }
        }
    }
    $entries.Sort([StringComparer]::Ordinal)
    return ($entries -join "`n")
}

function Get-UpdatedPath([string]$Value) {
    $managed = '^' + [regex]::Escape($InstallDir) + '\\nagi-\d+\.\d+\.\d+-windows-x86_64$'
    $parts = @($Value -split ';' | Where-Object {
        $part = $_.Trim().Trim('"').TrimEnd('\', '/').Replace('/', '\')
        $_ -and $part -ine $current -and $part -notmatch $managed
    })
    return (@($current) + $parts -join ';')
}

try {
    try { $lock = [IO.File]::Open((Join-Path $InstallDir '.install.lock'), 'OpenOrCreate', 'ReadWrite', 'None') }
    catch { throw "Another installer is running, or the install directory is not writable: $InstallDir" }
    New-Item -ItemType Directory -Path $work | Out-Null
    [Net.ServicePointManager]::SecurityProtocol = $previousTls -bor [Net.SecurityProtocolType]::Tls12
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    if ($Version -eq 'latest') {
        # The publisher reserves Latest for Nagi, not the VS Code extension.
        $response = Invoke-WebRequest -UseBasicParsing -Method Head -Uri 'https://github.com/disnana/Nagi/releases/latest'
        $uri = $response.BaseResponse.ResponseUri
        if (-not $uri) { $uri = $response.BaseResponse.RequestMessage.RequestUri }
        if ($uri.AbsoluteUri -notmatch '^https://github\.com/disnana/Nagi/releases/tag/nagi-v(\d+\.\d+\.\d+)$') {
            throw 'Latest is not a formal Nagi release; use -Version X.Y.Z'
        }
        $Version = $Matches[1]
    }
    $existing = Get-Item -Force -LiteralPath $current -ErrorAction SilentlyContinue
    if ($existing) {
        $managed = '^' + [regex]::Escape($InstallDir) + '\\nagi-\d+\.\d+\.\d+-windows-x86_64$'
        $targets = @($existing.Target)
        if ($existing.LinkType -ne 'Junction' -or $targets.Count -ne 1 -or $targets[0] -notmatch $managed) {
            throw "Existing path not overwritten: $current"
        }
    }
    $stem = "nagi-$Version-windows-x86_64"
    $destination = Join-Path $InstallDir $stem
    $root = Get-Distribution $Version (Join-Path $work 'new')
    $snapshot = Get-InstallSnapshot $root
    $actual = & (Join-Path $root 'nagic.exe') --version
    if ($LASTEXITCODE -ne 0 -or $actual -ne "nagic $Version") { throw 'Compiler version mismatch' }
    if (Test-Path -LiteralPath $destination) {
        if ((Get-InstallSnapshot $destination) -cne $snapshot) { throw "Existing install differs: $destination (not overwritten)" }
    } else {
        Move-Item -LiteralPath $root -Destination $destination
        $createdDestination = $true
    }
    # A junction needs no administrator/developer mode and keeps a real .exe
    # on PATH for editor subprocesses. Preserve the previous link until verified.
    New-Item -ItemType Junction -Path $next -Target $destination | Out-Null
    if ($existing) { Move-Item -LiteralPath $current -Destination $backup }
    $activated = $true
    Move-Item -LiteralPath $next -Destination $current
    $env:Path = Get-UpdatedPath $previousPath
    $command = Get-Command nagic -CommandType Application -ErrorAction Stop | Select-Object -First 1
    if ($command.Source -ine (Join-Path $current 'nagic.exe')) { throw 'PATH did not select the updated compiler' }
    $actual = & $command.Source --version
    if ($LASTEXITCODE -ne 0 -or $actual -ne "nagic $Version") { throw 'Activated compiler failed; restoring the previous version' }
    if (-not $NoPath) {
        $userPath = Get-UpdatedPath $previousUserPath
        $pathChanged = $true
        [Environment]::SetEnvironmentVariable('Path', $userPath, 'User')
        if ([Environment]::GetEnvironmentVariable('Path', 'User') -ne $userPath) { throw 'Could not save User PATH' }
    }
    $committed = $true
    if (Test-Path -LiteralPath $backup) { [IO.Directory]::Delete($backup) }
    foreach ($old in Get-ChildItem -Force -LiteralPath $InstallDir -Directory) {
        if ($old.FullName -ieq $destination -or $old.Name -notmatch '^nagi-(\d+\.\d+\.\d+)-windows-x86_64$') { continue }
        $oldVersion = $Matches[1]
        try {
            # Original 0.1.6 installs have no receipt. Compare the full tree to
            # the verified published archive before deleting any older version.
            $oldSnapshot = Get-InstallSnapshot $old.FullName
            $reference = Get-Distribution $oldVersion (Join-Path $work "old-$oldVersion")
            if ((Get-InstallSnapshot $reference) -cne $oldSnapshot -or (Get-InstallSnapshot $old.FullName) -cne $oldSnapshot) { throw 'Files were added or changed' }
            $handles = New-Object 'System.Collections.Generic.List[System.IDisposable]'
            try {
                # Detect currently busy files before removing any of the tree.
                foreach ($file in Get-ChildItem -Force -LiteralPath $old.FullName -Recurse -File) {
                    $handles.Add([IO.File]::Open($file.FullName, 'Open', 'Read', 'Delete'))
                }
            } finally { foreach ($handle in $handles) { $handle.Dispose() } }
            Remove-Item -LiteralPath $old.FullName -Recurse -Force
            Write-Host "Removed old version: $($old.FullName)"
        } catch { Write-Warning "Kept old installation: $($old.FullName) ($($_.Exception.Message))" }
    }
    Write-Host ''
    Write-InstallStatus "[OK] Installed Nagi $Version (prebuilt compiler)." Green
    Write-Host "     Location: $destination"
    Write-Host "     Command: $(Join-Path $current 'nagic.exe')"
    Write-Host ''
    Write-InstallStatus '[NEXT] Try in this terminal: nagic --version' Cyan
    if (-not $NoPath) { Write-Host '       Restart VS Code to refresh its PATH.' }
    Write-Host ''
    Write-Host '[INFO] To build your own Nagi apps with nagic build/run:'
    Write-Host '       Rust/Cargo and Visual Studio C++ Build Tools are required.'
    Write-Host '       Use your existing installation if these tools are already installed.'
    if ($env:NAGI_ROOT) { Write-Warning 'NAGI_ROOT is set; remove it to use the installed runtime automatically.' }
} finally {
    if (-not $committed -and $activated) {
        if (Test-Path -LiteralPath $current) { [IO.Directory]::Delete($current) }
        if (Test-Path -LiteralPath $backup) { Move-Item -LiteralPath $backup -Destination $current }
    }
    if (-not $committed) {
        $env:Path = $previousPath
        if ($pathChanged -and [Environment]::GetEnvironmentVariable('Path', 'User') -ne $previousUserPath) {
            [Environment]::SetEnvironmentVariable('Path', $previousUserPath, 'User')
        }
        if ($createdDestination) {
            try {
                if ((Get-InstallSnapshot $destination) -cne $snapshot) { throw 'Files changed during installation' }
                Remove-Item -LiteralPath $destination -Recurse -Force
            } catch { Write-Warning "Kept incomplete installation: $destination ($($_.Exception.Message))" }
        }
    }
    [Net.ServicePointManager]::SecurityProtocol = $previousTls
    # Remove junctions themselves, never recurse through them into a version.
    foreach ($junction in @($next, $backup)) {
        if (Test-Path -LiteralPath $junction) { [IO.Directory]::Delete($junction) }
    }
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force }
    if ($lock) { $lock.Dispose() }
}
