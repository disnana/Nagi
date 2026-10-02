param(
    [Parameter(Mandatory)][string]$Archive,
    [string]$Version = '0.1.6',
    [switch]$TestUserPath
)
$ErrorActionPreference = 'Stop'
$installer = Join-Path $PSScriptRoot '..\install.ps1'
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('nagi installer space ' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temporary | Out-Null
$assets = Join-Path $temporary 'assets'
New-Item -ItemType Directory -Path $assets | Out-Null
Copy-Item -LiteralPath $Archive, "$Archive.sha256" -Destination $assets
$downloadFixture = [pscustomobject]@{
    Assets = $assets
    BadChecksum = $false
    LatestUri = "https://github.com/disnana/Nagi/releases/tag/nagi-v$Version"
}
$previousPath = $env:Path
$previousUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$previousRoot = $env:NAGI_ROOT
$previousNative = $env:NAGI_NATIVE_TARGET_DIR
Set-Item -Path Function:Invoke-WebRequest -Value ({
    param($Uri, $OutFile, $Method, [switch]$UseBasicParsing)
    if ($Method -eq 'Head') {
        if ($Uri -ne 'https://github.com/disnana/Nagi/releases/latest') { throw "Unexpected latest URL: $Uri" }
        return [pscustomobject]@{ BaseResponse = [pscustomobject]@{ ResponseUri = [uri]$downloadFixture.LatestUri } }
    }
    if ($Uri -notmatch '^https://github\.com/disnana/Nagi/releases/download/nagi-v\d+\.\d+\.\d+/([^/]+)$') { throw "Unexpected download URL: $Uri" }
    Copy-Item -LiteralPath (Join-Path $downloadFixture.Assets $Matches[1]) -Destination $OutFile
    if ($downloadFixture.BadChecksum -and $Uri.EndsWith('.sha256')) {
        [IO.File]::WriteAllText($OutFile, (('0' * 64) + '  ' + [IO.Path]::GetFileName($OutFile).Replace('.sha256', '') + "`n"))
    }
}.GetNewClosure())

function New-Fixture([string]$Release, [switch]$FailActivation) {
    $stem = "nagi-$Release-windows-x86_64"
    $folder = Join-Path $temporary $stem
    New-Item -ItemType Directory -Path (Join-Path $folder 'runtime\src') -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $folder 'runtime\Cargo.toml'), 'runtime fixture')
    [IO.File]::WriteAllText((Join-Path $folder 'runtime\src\lib.rs'), 'runtime fixture')
    foreach ($name in @('LICENSE', 'README.txt')) { [IO.File]::WriteAllText((Join-Path $folder $name), 'fixture') }
    @{ version = $Release; platform = 'windows-x86_64'; commit = ('a' * 40) } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $folder 'release.json') -Encoding UTF8
    $activation = if ($FailActivation) { 'if !std::env::current_exe().unwrap().to_string_lossy().contains(".install.") { std::process::exit(73); }' } else { '' }
    $source = Join-Path $temporary 'fixture.rs'
    [IO.File]::WriteAllText($source, ('fn main() { ' + $activation + ' println!("nagic ' + $Release + '"); }'))
    & rustc --crate-name nagi_installer_fixture --edition 2021 $source -o (Join-Path $folder 'nagic.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Could not build installer fixture' }
    # rustc's PDB is not part of the fixture distribution.
    Get-ChildItem -LiteralPath $folder -Filter '*.pdb' | Remove-Item
    $zip = Join-Path $assets "$stem.zip"
    Compress-Archive -LiteralPath $folder -DestinationPath $zip
    [IO.File]::WriteAllText("$zip.sha256", (Get-FileHash -LiteralPath $zip).Hash.ToLowerInvariant() + "  $stem.zip`n")
}

function Assert-Active([string]$Directory, [string]$Release) {
    $output = & nagic --version
    if ($LASTEXITCODE -ne 0 -or $output -ne "nagic $Release") { throw "Wrong active compiler: $output" }
    $command = Get-Command nagic -CommandType Application | Select-Object -First 1
    if ($command.Source -ine (Join-Path $Directory 'current\nagic.exe')) { throw 'Compiler did not use the fixed PATH entry' }
}

try {
    $installDir = Join-Path $temporary 'versions'
    $options = @{ InstallDir = $installDir; NoPath = -not $TestUserPath }
    & $installer @options
    & $installer @options
    Assert-Active $installDir $Version
    $current = Join-Path $installDir 'current'
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($TestUserPath) {
        if (($userPath -split ';' | Where-Object { $_ -ieq $current }).Count -ne 1 -or ($userPath -split ';')[0] -ine $current) { throw 'User PATH should start with exactly one fixed entry' }
    } elseif ($userPath -ne $previousUserPath) { throw '-NoPath changed User PATH' }
    # Exercise the real compiler/runtime through the junction, outside checkout.
    $project = Join-Path $temporary 'outside project'
    New-Item -ItemType Directory -Path $project | Out-Null
    [IO.File]::WriteAllText((Join-Path $project 'main.nagi'), "def main():`n    print(42)`n")
    [IO.File]::WriteAllText((Join-Path $project 'nagi.toml'), "entry='main.nagi'`n")
    Remove-Item Env:NAGI_ROOT -ErrorAction SilentlyContinue
    $env:NAGI_NATIVE_TARGET_DIR = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\native-target'))
    $result = & nagic run --project (Join-Path $project 'nagi.toml')
    if ($LASTEXITCODE -ne 0 -or @($result)[-1] -ne '42') { throw 'Installed compiler could not run an external project' }

    New-Fixture '0.0.1'
    New-Fixture '0.0.2'
    New-Fixture '0.0.3' -FailActivation
    $downloadFixture.LatestUri = 'https://github.com/disnana/Nagi/releases/tag/nagi-v0.0.2'
    & $installer @options -Version '0.0.1'
    & $installer @options
    Assert-Active $installDir '0.0.2'
    if (@(Get-ChildItem -LiteralPath $installDir -Filter 'nagi-*' -Directory).Count -ne 1) { throw 'Old versions accumulated after upgrade' }
    & $installer @options
    & $installer @options -Version '0.0.1'
    Assert-Active $installDir '0.0.1'
    if (Test-Path -LiteralPath (Join-Path $installDir 'nagi-0.0.2-windows-x86_64')) { throw 'Explicit version did not replace the previous install' }

    # Download and post-switch failures preserve the original command and PATH.
    $savedPath = $env:Path
    $savedUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    foreach ($failure in @('checksum', 'activation')) {
        $downloadFixture.BadChecksum = $failure -eq 'checksum'
        $failed = $false
        try { & $installer @options -Version '0.0.3' }
        catch {
            if ($_.Exception.Message -notmatch 'SHA-256 mismatch|Activated compiler failed') { throw }
            $failed = $true
        }
        if (-not $failed -or $env:Path -ne $savedPath -or [Environment]::GetEnvironmentVariable('Path', 'User') -ne $savedUserPath) { throw 'Failure did not restore PATH' }
        Assert-Active $installDir '0.0.1'
    }
    $downloadFixture.BadChecksum = $false
    & $installer @options

    # Original installer layout: direct version on PATH, no current junction.
    $legacy = Join-Path $temporary 'legacy'
    New-Item -ItemType Directory -Path $legacy | Out-Null
    Expand-Archive -LiteralPath (Join-Path $assets 'nagi-0.0.1-windows-x86_64.zip') -DestinationPath $legacy
    $legacyPath = Join-Path $legacy 'nagi-0.0.1-windows-x86_64'
    $env:Path = "$legacyPath;$legacyPath\;$env:Path"
    if ($TestUserPath) { [Environment]::SetEnvironmentVariable('Path', "$legacyPath;$legacyPath\;$savedUserPath", 'User') }
    & $installer -InstallDir $legacy -NoPath:(-not $TestUserPath)
    Assert-Active $legacy '0.0.2'
    if (Test-Path -LiteralPath $legacyPath) { throw 'Original installer version was not cleaned up' }
    foreach ($value in @($env:Path, [Environment]::GetEnvironmentVariable('Path', 'User'))) {
        if ($value -like "*$legacyPath*") { throw 'Legacy PATH entries remain' }
    }

    # Added, changed, and in-use files must preserve the entire old tree.
    foreach ($kind in @('changed', 'added', 'empty-directory', 'busy', 'junction')) {
        $protected = Join-Path $temporary "protected-$kind"
        & $installer -InstallDir $protected -Version '0.0.1' -NoPath
        $old = Join-Path $protected 'nagi-0.0.1-windows-x86_64'
        $handle = $null
        if ($kind -eq 'changed') { [IO.File]::WriteAllText((Join-Path $old 'runtime\src\lib.rs'), 'user changes') }
        if ($kind -eq 'added') { [IO.File]::WriteAllText((Join-Path $old 'notes.txt'), 'user notes') }
        if ($kind -eq 'empty-directory') { New-Item -ItemType Directory -Path (Join-Path $old 'project') | Out-Null }
        if ($kind -eq 'busy') { $handle = [IO.File]::Open((Join-Path $old 'nagic.exe'), 'Open', 'Read', 'None') }
        if ($kind -eq 'junction') { New-Item -ItemType Junction -Path (Join-Path $old 'outside') -Target $assets | Out-Null }
        try { & $installer -InstallDir $protected -NoPath }
        finally { if ($handle) { $handle.Dispose() } }
        Assert-Active $protected '0.0.2'
        if (-not (Test-Path -LiteralPath (Join-Path $old 'runtime\src\lib.rs'))) { throw "Protected old version was removed: $kind" }
        if ($kind -eq 'junction') { [IO.Directory]::Delete((Join-Path $old 'outside')) }
    }

    $lock = [IO.File]::Open((Join-Path $legacy '.install.lock'), 'Open', 'ReadWrite', 'None')
    try {
        $failed = $false
        try { & $installer -InstallDir $legacy -NoPath } catch { $failed = $_.Exception.Message -match 'Another installer' }
        if (-not $failed) { throw 'Concurrent installation was not rejected' }
    } finally { $lock.Dispose() }
    foreach ($tag in @('vscode-v0.1.8', 'nagi-v0.2.0-beta')) {
        $downloadFixture.LatestUri = "https://github.com/disnana/Nagi/releases/tag/$tag"
        $failed = $false
        try { & $installer -InstallDir $legacy -NoPath } catch { $failed = $_.Exception.Message -match 'Latest is not' }
        if (-not $failed) { throw "Invalid latest release was accepted: $tag" }
    }
    Write-Host 'PowerShell installer: external project, upgrade, repeat, explicit version, rollback, legacy PATH, protected files, lock, and latest selection passed.'
} finally {
    $env:Path = $previousPath
    $env:NAGI_ROOT = $previousRoot
    $env:NAGI_NATIVE_TARGET_DIR = $previousNative
    if ($TestUserPath) { [Environment]::SetEnvironmentVariable('Path', $previousUserPath, 'User') }
    # Delete installer junctions before recursively removing the temporary tree.
    Get-ChildItem -LiteralPath $temporary -Directory | ForEach-Object {
        $junction = Join-Path $_.FullName 'current'
        if (Test-Path -LiteralPath $junction) { [IO.Directory]::Delete($junction) }
    }
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
