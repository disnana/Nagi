param(
    [Parameter(Mandatory)][string]$Archive,
    [string]$Version = '0.1.6',
    [switch]$TestUserPath
)
$ErrorActionPreference = 'Stop'
$downloadFixture = [pscustomobject]@{
    Archive = [IO.Path]::GetFullPath($Archive)
    BadChecksum = $false
}
$installer = Join-Path $PSScriptRoot '..\install.ps1'
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('nagi installer space ' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temporary | Out-Null
$previousPath = $env:Path
$previousUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
# Capture fixture state so a child script cannot rebind it to its script scope.
Set-Item -Path Function:Invoke-WebRequest -Value ({
    param($Uri, $OutFile, [switch]$UseBasicParsing)
    $source = if ($Uri.EndsWith('.sha256')) { "$($downloadFixture.Archive).sha256" } else { $downloadFixture.Archive }
    Copy-Item -LiteralPath $source -Destination $OutFile
    if ($downloadFixture.BadChecksum -and $Uri.EndsWith('.sha256')) {
        $name = [IO.Path]::GetFileName($downloadFixture.Archive)
        [IO.File]::WriteAllText($OutFile, (('0' * 64) + "  $name`n"))
    }
}.GetNewClosure())
try {
    $installDir = Join-Path $temporary 'versions'
    $options = @{ Version = $Version; InstallDir = $installDir; NoPath = -not $TestUserPath }
    & $installer @options
    & $installer @options
    $destination = Join-Path $installDir "nagi-$Version-windows-x86_64"
    $output = & nagic --version
    if ($LASTEXITCODE -ne 0 -or $output -ne "nagic $Version") { throw 'Installed compiler was not found on PATH' }
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($TestUserPath) {
        if (($userPath -split ';' | Where-Object { $_ -ieq $destination }).Count -ne 1) { throw 'User PATH should contain the install once' }
        if (($userPath -split ';')[0] -ine $destination) { throw 'New compiler should be first in User PATH' }
    } elseif ($userPath -ne $previousUserPath) { throw '-NoPath changed the persistent User PATH' }
    $beforeFailure = $env:Path
    $downloadFixture.BadChecksum = $true
    $rejected = $false
    try { & $installer -Version $Version -InstallDir (Join-Path $temporary 'bad') -NoPath }
    catch { if ($_.Exception.Message -notmatch 'SHA-256 mismatch') { throw }; $rejected = $true }
    if (-not $rejected -or $env:Path -ne $beforeFailure) { throw 'Checksum failure should leave PATH unchanged' }
    if (Test-Path (Join-Path $temporary "bad\nagi-$Version-windows-x86_64")) { throw 'Invalid archive was installed' }
    Write-Host 'PowerShell installer: install, repeat, PATH, and checksum failure passed.'
} finally {
    $env:Path = $previousPath
    if ($TestUserPath) { [Environment]::SetEnvironmentVariable('Path', $previousUserPath, 'User') }
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
