[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Run the uninstaller tests on Windows.' }
$uninstaller = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\uninstall.ps1'))
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('nagi uninstaller space ' + [guid]::NewGuid())
$assets = Join-Path $temporary 'assets'
$sources = Join-Path $temporary 'sources'
$previousPath = $env:Path
$previousUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$previousTls = [Net.ServicePointManager]::SecurityProtocol
$ownedJunctions = New-Object 'System.Collections.Generic.List[string]'
$fixture = [pscustomobject]@{ Assets = $assets; BadChecksum = $false; Offline = $false; Downloads = 0 }

# The production script is also exercised unchanged with -NoPath. For PATH
# cases, replace only the Environment dependency so no test writes the registry.
if (-not ('NagiUninstallTestEnvironment' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
public static class NagiUninstallTestEnvironment {
    public static string UserPath;
    public static int Writes;
    public static void ResetUserPathToNull() { UserPath = null; }
    public static string GetEnvironmentVariable(string name, EnvironmentVariableTarget target) {
        if (name != "Path" || target != EnvironmentVariableTarget.User)
            throw new InvalidOperationException("Unexpected environment read in uninstaller test");
        return UserPath;
    }
    public static void SetEnvironmentVariable(string name, string value, EnvironmentVariableTarget target) {
        if (name != "Path" || target != EnvironmentVariableTarget.User)
            throw new InvalidOperationException("Unexpected environment write in uninstaller test");
        UserPath = value;
        Writes++;
    }
}
'@
}

function Assert([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function New-Junction([string]$Path, [string]$Target) {
    New-Item -ItemType Junction -Path $Path -Target $Target | Out-Null
    $ownedJunctions.Add($Path)
}

function New-Release([string]$Version, [string]$Kind = 'normal') {
    $stem = "nagi-$Version-windows-x86_64"
    $root = Join-Path $sources $stem
    New-Item -ItemType Directory -Path (Join-Path $root 'runtime\src') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $root 'empty') | Out-Null
    [IO.File]::WriteAllBytes((Join-Path $root 'nagic.exe'), [byte[]](0, 78, 65, 71, 73, 255))
    foreach ($name in @('LICENSE', 'README.txt', '.hidden.txt', 'runtime\Cargo.toml', 'runtime\src\lib.rs')) {
        [IO.File]::WriteAllText((Join-Path $root $name), "fixture $name $Version")
    }
    $metadataVersion = if ($Kind -eq 'metadata') { '9.9.9' } else { $Version }
    @{ version = $metadataVersion; platform = 'windows-x86_64'; commit = ('a' * 40) } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'release.json') -Encoding UTF8
    if ($Kind -eq 'incomplete') { Remove-Item -LiteralPath (Join-Path $root 'runtime\src\lib.rs') }
    $zip = Join-Path $assets "$stem.zip"
    # Compress-Archive omits hidden files; the .NET API includes the full tree.
    [IO.Compression.ZipFile]::CreateFromDirectory($root, $zip, [IO.Compression.CompressionLevel]::Optimal, $true)
    if ($Kind -eq 'unsafe') {
        $archive = [IO.Compression.ZipFile]::Open($zip, [IO.Compression.ZipArchiveMode]::Update)
        try {
            $entry = $archive.CreateEntry("$stem/../escaped.txt")
            $writer = [IO.StreamWriter]::new($entry.Open())
            try { $writer.Write('must never be extracted') } finally { $writer.Dispose() }
        } finally { $archive.Dispose() }
    }
    [IO.File]::WriteAllText("$zip.sha256", ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant() + "  $stem.zip`n"))
}

function New-Install([string]$Case, [string]$Version = '0.0.1', [switch]$Legacy) {
    $directory = Join-Path $temporary $Case
    New-Item -ItemType Directory -Path $directory | Out-Null
    $source = Join-Path $sources "nagi-$Version-windows-x86_64"
    Copy-Item -LiteralPath $source -Destination $directory -Recurse
    $versionDirectory = Join-Path $directory "nagi-$Version-windows-x86_64"
    # Get-ChildItem must include hidden files when verifying/removing releases.
    $hidden = Join-Path $versionDirectory '.hidden.txt'
    [IO.File]::SetAttributes($hidden, ([IO.File]::GetAttributes($hidden) -bor [IO.FileAttributes]::Hidden))
    $current = Join-Path $directory 'current'
    if (-not $Legacy) { New-Junction $current $versionDirectory }
    return [pscustomobject]@{ Directory = $directory; VersionDirectory = $versionDirectory; Current = $current }
}

function Get-TreeState([string]$Root) {
    $pending = New-Object 'System.Collections.Generic.Queue[string]'
    $entries = New-Object 'System.Collections.Generic.List[string]'
    $pending.Enqueue($Root)
    while ($pending.Count) {
        foreach ($child in Get-ChildItem -LiteralPath $pending.Dequeue() -Force) {
            $relative = $child.FullName.Substring($Root.Length + 1)
            if ($child.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                $entries.Add("R $relative $($child.LinkType) $(@($child.Target) -join ',')")
            } elseif ($child.PSIsContainer) {
                $entries.Add("D $relative")
                $pending.Enqueue($child.FullName)
            } else {
                $entries.Add("F $relative $((Get-FileHash -LiteralPath $child.FullName -Algorithm SHA256).Hash)")
            }
        }
    }
    $entries.Sort([StringComparer]::Ordinal)
    return ($entries -join "`n")
}

function Invoke-Uninstall([string]$Directory, [switch]$WhatIf, [switch]$FixturePath) {
    $warnings = @()
    $options = @{ InstallDir = $Directory; NoPath = -not $FixturePath; Confirm = $false; WarningVariable = 'warnings' }
    if ($WhatIf) { $options.WhatIf = $true }
    if ($FixturePath) { & $mockedUninstaller @options } else { & $uninstaller @options }
    $script:lastUninstallWarnings = @($warnings)
    Assert ([Environment]::GetEnvironmentVariable('Path', 'User') -ceq $previousUserPath) 'The test changed the actual User PATH'
}

Set-Item -Path Function:Invoke-WebRequest -Value ({
    param($Uri, $OutFile, $Method, [switch]$UseBasicParsing)
    if ($fixture.Offline) { throw 'Fixture download unavailable' }
    if ($Method -or $Uri -notmatch '^https://github\.com/disnana/Nagi/releases/download/nagi-v\d+\.\d+\.\d+/([^/]+)$') {
        throw "Unexpected uninstaller download: $Uri"
    }
    $fixture.Downloads++
    $name = $Matches[1]
    Copy-Item -LiteralPath (Join-Path $fixture.Assets $name) -Destination $OutFile -WhatIf:$false
    if ($fixture.BadChecksum -and $name.EndsWith('.sha256')) {
        [IO.File]::WriteAllText($OutFile, (('0' * 64) + '  ' + $name.Substring(0, $name.Length - 7) + "`n"))
    }
}.GetNewClosure())

try {
    New-Item -ItemType Directory -Path $assets, $sources -Force | Out-Null
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $scriptSource = Get-Content -LiteralPath $uninstaller -Raw
    $getToken = '[Environment]::GetEnvironmentVariable'
    $setToken = '[Environment]::SetEnvironmentVariable'
    $getCount = ([regex]::Matches($scriptSource, [regex]::Escape($getToken))).Count
    $setCount = ([regex]::Matches($scriptSource, [regex]::Escape($setToken))).Count
    # Keep these counts synchronized deliberately if the production dependency
    # changes; an unrecognized registry call must not run in a fixture test.
    Assert ($getCount -eq 2 -and $setCount -eq 1) 'Unexpected Environment dependency count; review the registry isolation before updating this test'
    Assert ($scriptSource -notmatch '\[System\.Environment\]') 'Unrecognized Environment type bypasses registry isolation'
    $mockedUninstaller = [scriptblock]::Create($scriptSource.Replace($getToken, '[NagiUninstallTestEnvironment]::GetEnvironmentVariable').Replace($setToken, '[NagiUninstallTestEnvironment]::SetEnvironmentVariable'))

    New-Release '0.0.1'
    New-Release '0.0.2'
    New-Release '0.0.90' 'unsafe'
    New-Release '0.0.91' 'incomplete'
    New-Release '0.0.92' 'metadata'

    # Preview must preserve links, complete trees, and both PATH values.
    $preview = New-Install 'preview'
    $env:Path = "$($preview.Current);$previousPath"
    [NagiUninstallTestEnvironment]::UserPath = "$($preview.Current);keep-user"
    [NagiUninstallTestEnvironment]::Writes = 0
    $before = Get-TreeState $preview.Directory
    $previewPath = $env:Path
    $previewUserPath = [NagiUninstallTestEnvironment]::UserPath
    $previewDownloads = $fixture.Downloads
    Invoke-Uninstall $preview.Directory -WhatIf -FixturePath
    Assert ($lastUninstallWarnings.Count -eq 0) '-WhatIf failed to verify the pristine release'
    Assert ($fixture.Downloads -eq $previewDownloads + 2) '-WhatIf did not download both pristine verification assets'
    Assert ((Get-TreeState $preview.Directory) -ceq $before) '-WhatIf changed the installation tree'
    Assert ($env:Path -ceq $previewPath) '-WhatIf changed process PATH'
    Assert ([NagiUninstallTestEnvironment]::UserPath -ceq $previewUserPath -and [NagiUninstallTestEnvironment]::Writes -eq 0) '-WhatIf changed User PATH'

    # The unmodified production script must obey -NoPath, including a repeat.
    $normal = New-Install 'normal'
    $env:Path = "$($normal.Current);$previousPath"
    Invoke-Uninstall $normal.Directory
    Assert ($env:Path -ceq $previousPath) '-NoPath did not clean the process PATH'
    Assert (-not (Test-Path -LiteralPath $normal.Current)) 'Owned current junction remains'
    Assert (-not (Test-Path -LiteralPath $normal.VersionDirectory)) 'Verified release remains'
    Assert (Test-Path -LiteralPath $normal.Directory -PathType Container) 'Install root was removed'
    Assert (Test-Path -LiteralPath (Join-Path $normal.Directory '.install.lock') -PathType Leaf) 'Shared lock file was removed'
    Invoke-Uninstall $normal.Directory
    Assert (Test-Path -LiteralPath $normal.Directory) 'Repeat uninstall removed install root'
    $missing = Join-Path $temporary 'not installed'
    Invoke-Uninstall $missing
    Assert (-not (Test-Path -LiteralPath $missing)) 'Uninstall created a missing install root'

    # A verified original layout has no current junction and can still be removed.
    $legacy = New-Install 'legacy' -Legacy
    Invoke-Uninstall $legacy.Directory
    Assert (-not (Test-Path -LiteralPath $legacy.VersionDirectory)) 'Verified legacy layout remains'

    # Full-tree mismatches preserve the entire version, even when current detaches.
    foreach ($kind in @('changed', 'added', 'empty-directory', 'hidden', 'busy', 'junction')) {
        $protected = New-Install "protected-$kind"
        $handle = $null
        $outside = Join-Path $temporary "outside-$kind"
        if ($kind -eq 'changed') { [IO.File]::WriteAllText((Join-Path $protected.VersionDirectory 'runtime\src\lib.rs'), 'user changes') }
        if ($kind -eq 'added') { [IO.File]::WriteAllText((Join-Path $protected.VersionDirectory 'notes.txt'), 'user notes') }
        if ($kind -eq 'empty-directory') { New-Item -ItemType Directory -Path (Join-Path $protected.VersionDirectory 'project') | Out-Null }
        if ($kind -eq 'hidden') {
            $hidden = Join-Path $protected.VersionDirectory '.user-notes'
            [IO.File]::WriteAllText($hidden, 'hidden user notes')
            [IO.File]::SetAttributes($hidden, [IO.FileAttributes]::Hidden)
        }
        if ($kind -eq 'junction') {
            New-Item -ItemType Directory -Path $outside | Out-Null
            [IO.File]::WriteAllText((Join-Path $outside 'sentinel.txt'), 'outside user files')
            New-Junction (Join-Path $protected.VersionDirectory 'outside') $outside
        }
        $before = Get-TreeState $protected.VersionDirectory
        if ($kind -eq 'busy') { $handle = [IO.File]::Open((Join-Path $protected.VersionDirectory 'nagic.exe'), 'Open', 'Read', 'None') }
        try { Invoke-Uninstall $protected.Directory }
        finally { if ($handle) { $handle.Dispose() } }
        Assert (Test-Path -LiteralPath $protected.VersionDirectory -PathType Container) "Protected version removed: $kind"
        Assert ((Get-TreeState $protected.VersionDirectory) -ceq $before) "Protected version changed: $kind"
        Assert (-not (Test-Path -LiteralPath $protected.Current)) "Owned current did not detach from protected version: $kind"
        if ($kind -eq 'junction') {
            Assert ((Get-Content -LiteralPath (Join-Path $outside 'sentinel.txt') -Raw) -ceq 'outside user files') 'Descendant junction target changed'
        }
    }

    # Foreign current paths are user-owned, including a similarly named external release.
    foreach ($kind in @('file', 'directory', 'foreign-junction')) {
        $foreign = New-Install "foreign-$kind" -Legacy
        if ($kind -eq 'file') { [IO.File]::WriteAllText($foreign.Current, 'current user file') }
        if ($kind -eq 'directory') {
            New-Item -ItemType Directory -Path $foreign.Current | Out-Null
            [IO.File]::WriteAllText((Join-Path $foreign.Current 'notes.txt'), 'current user directory')
        }
        if ($kind -eq 'foreign-junction') {
            $outside = Join-Path $temporary 'foreign-target\nagi-0.0.1-windows-x86_64'
            New-Item -ItemType Directory -Path $outside -Force | Out-Null
            [IO.File]::WriteAllText((Join-Path $outside 'sentinel.txt'), 'foreign target')
            New-Junction $foreign.Current $outside
        }
        Invoke-Uninstall $foreign.Directory
        $currentItem = Get-Item -LiteralPath $foreign.Current -Force -ErrorAction SilentlyContinue
        Assert ($null -ne $currentItem) "Foreign current removed: $kind"
        if ($kind -eq 'file') { Assert ((Get-Content -LiteralPath $foreign.Current -Raw) -ceq 'current user file') 'Current file changed' }
        if ($kind -eq 'directory') { Assert ((Get-Content -LiteralPath (Join-Path $foreign.Current 'notes.txt') -Raw) -ceq 'current user directory') 'Current directory changed' }
        if ($kind -eq 'foreign-junction') { Assert ((Get-Content -LiteralPath (Join-Path $outside 'sentinel.txt') -Raw) -ceq 'foreign target') 'Foreign current target changed' }
    }

    $versionLink = New-Install 'version-junction' -Legacy
    $versionTarget = Join-Path $temporary 'version-junction-target'
    [IO.Directory]::Move($versionLink.VersionDirectory, $versionTarget)
    New-Junction $versionLink.VersionDirectory $versionTarget
    $before = Get-TreeState $versionTarget
    Invoke-Uninstall $versionLink.Directory
    Assert ((Get-Item -LiteralPath $versionLink.VersionDirectory -Force).LinkType -eq 'Junction') 'Version junction was removed'
    Assert ((Get-TreeState $versionTarget) -ceq $before) 'Version junction target changed'

    # Unverifiable downloads must retain every installed byte and empty directory.
    foreach ($kind in @('checksum', 'offline', 'unsafe', 'incomplete', 'metadata')) {
        $version = switch ($kind) {
            'unsafe' { '0.0.90' }
            'incomplete' { '0.0.91' }
            'metadata' { '0.0.92' }
            default { '0.0.1' }
        }
        $unverified = New-Install "unverified-$kind" $version
        $before = Get-TreeState $unverified.VersionDirectory
        $fixture.BadChecksum = $kind -eq 'checksum'
        $fixture.Offline = $kind -eq 'offline'
        try { Invoke-Uninstall $unverified.Directory }
        finally { $fixture.BadChecksum = $false; $fixture.Offline = $false }
        $expectedWarning = switch ($kind) {
            'checksum' { 'SHA-256 mismatch' }
            'offline' { 'Fixture download unavailable' }
            'unsafe' { 'Unsafe archive entry' }
            'incomplete' { 'Incomplete distribution' }
            'metadata' { 'Distribution metadata mismatch' }
        }
        Assert ((($lastUninstallWarnings | ForEach-Object { $_.ToString() }) -join "`n") -match [regex]::Escape($expectedWarning)) "Wrong verification failure for $kind"
        Assert (Test-Path -LiteralPath $unverified.VersionDirectory) "Unverifiable version removed: $kind"
        Assert ((Get-TreeState $unverified.VersionDirectory) -ceq $before) "Unverifiable version changed: $kind"
        Assert (-not (Test-Path -LiteralPath $unverified.Current)) "Owned current did not detach from unverifiable version: $kind"
        Assert (-not (Test-Path -LiteralPath (Join-Path $temporary 'escaped.txt'))) 'Unsafe archive escaped the verification directory'
    }

    # Refuse a shared installer lock before detaching current or changing PATH.
    $locked = New-Install 'locked'
    $lockPath = Join-Path $locked.Directory '.install.lock'
    $lock = [IO.File]::Open($lockPath, 'OpenOrCreate', 'ReadWrite', 'None')
    $failed = $false
    $lockProcessPath = $env:Path
    try {
        try { Invoke-Uninstall $locked.Directory } catch { $failed = $true }
    } finally { $lock.Dispose() }
    Assert $failed 'Concurrent installer lock was ignored'
    Assert (Test-Path -LiteralPath $locked.Current) 'Lock failure detached current'
    Assert (Test-Path -LiteralPath $locked.VersionDirectory) 'Lock failure removed release'
    Assert ($env:Path -ceq $lockProcessPath) 'Lock failure changed process PATH'

    # Never traverse an install-root junction into another installation.
    $rootTarget = New-Install 'root-target'
    $rootLink = Join-Path $temporary 'linked-root'
    New-Junction $rootLink $rootTarget.Directory
    $before = Get-TreeState $rootTarget.Directory
    try { Invoke-Uninstall $rootLink } catch { }
    Assert ((Get-TreeState $rootTarget.Directory) -ceq $before) 'Install-root junction target changed'
    Assert ((Get-Item -LiteralPath $rootLink -Force).LinkType -eq 'Junction') 'Install-root junction removed'

    # Registry isolation allows full PATH tests without saving any real User PATH.
    $pathCase = New-Install 'path'
    Copy-Item -LiteralPath (Join-Path $sources 'nagi-0.0.2-windows-x86_64') -Destination $pathCase.Directory -Recurse
    $otherVersion = Join-Path $pathCase.Directory 'nagi-0.0.2-windows-x86_64'
    $foreignPath = Join-Path $temporary 'other versions\nagi-0.0.1-windows-x86_64'
    $keep = "$foreignPath;$($pathCase.Current)\tools;keep-entry"
    $env:Path = "$($pathCase.Current);$($pathCase.Current)\;$($pathCase.VersionDirectory);$otherVersion;$keep"
    [NagiUninstallTestEnvironment]::UserPath = ('"' + $pathCase.Current.ToUpperInvariant() + '\";' + $pathCase.VersionDirectory.Replace('\', '/') + '/;' + $otherVersion + ';' + $keep)
    [NagiUninstallTestEnvironment]::Writes = 0
    Invoke-Uninstall $pathCase.Directory -FixturePath
    Assert ($env:Path -ceq $keep) 'Process PATH did not remove only owned current/verified legacy entries'
    Assert ([NagiUninstallTestEnvironment]::UserPath -ceq $keep) 'User PATH did not remove only owned current/verified legacy entries'
    Assert ([NagiUninstallTestEnvironment]::Writes -eq 1) 'User PATH should be saved exactly once when changed'

    $keptPath = New-Install 'path-unverified' -Legacy
    [IO.File]::WriteAllText((Join-Path $keptPath.VersionDirectory 'README.txt'), 'user content')
    $env:Path = "$($keptPath.VersionDirectory);keep-entry"
    [NagiUninstallTestEnvironment]::UserPath = $env:Path
    [NagiUninstallTestEnvironment]::Writes = 0
    Invoke-Uninstall $keptPath.Directory -FixturePath
    Assert ($env:Path -ceq "$($keptPath.VersionDirectory);keep-entry") 'Changed legacy release lost its process PATH'
    Assert ([NagiUninstallTestEnvironment]::UserPath -ceq $env:Path -and [NagiUninstallTestEnvironment]::Writes -eq 0) 'Changed legacy release lost its User PATH'

    $changedCurrent = New-Install 'path-changed-current'
    [IO.File]::WriteAllText((Join-Path $changedCurrent.VersionDirectory 'README.txt'), 'user content')
    $env:Path = "$($changedCurrent.Current);$($changedCurrent.VersionDirectory);keep-entry"
    [NagiUninstallTestEnvironment]::UserPath = $env:Path
    [NagiUninstallTestEnvironment]::Writes = 0
    Invoke-Uninstall $changedCurrent.Directory -FixturePath
    Assert ($env:Path -ceq "$($changedCurrent.VersionDirectory);keep-entry") 'Changed current did not remove only its owned junction PATH'
    Assert ([NagiUninstallTestEnvironment]::UserPath -ceq $env:Path -and [NagiUninstallTestEnvironment]::Writes -eq 1) 'Changed current did not clean only its owned User PATH'
    Assert (Test-Path -LiteralPath $changedCurrent.VersionDirectory) 'PATH cleanup removed changed current target'

    $foreignCurrentPath = New-Install 'path-foreign-current' -Legacy
    New-Item -ItemType Directory -Path $foreignCurrentPath.Current | Out-Null
    [IO.File]::WriteAllText((Join-Path $foreignCurrentPath.Current 'notes.txt'), 'user content')
    $env:Path = "$($foreignCurrentPath.Current);$($foreignCurrentPath.VersionDirectory);keep-entry"
    [NagiUninstallTestEnvironment]::UserPath = $env:Path
    [NagiUninstallTestEnvironment]::Writes = 0
    Invoke-Uninstall $foreignCurrentPath.Directory -FixturePath
    Assert ($env:Path -ceq "$($foreignCurrentPath.Current);keep-entry") 'Foreign current lost its process PATH'
    Assert ([NagiUninstallTestEnvironment]::UserPath -ceq $env:Path -and [NagiUninstallTestEnvironment]::Writes -eq 1) 'Foreign current lost its User PATH'

    foreach ($kind in @('missing', 'empty')) {
        $nullPathDirectory = Join-Path $temporary "null-path-$kind"
        if ($kind -eq 'empty') { New-Item -ItemType Directory -Path $nullPathDirectory | Out-Null }
        $env:Path = 'keep-entry'
        # PowerShell converts $null to String.Empty when assigning to a typed
        # C# string field. Reset from C# so this fixture models an absent PATH.
        [NagiUninstallTestEnvironment]::ResetUserPathToNull()
        Assert ($null -eq [NagiUninstallTestEnvironment]::UserPath) 'Fixture did not clear User PATH to null'
        [NagiUninstallTestEnvironment]::Writes = 0
        Invoke-Uninstall $nullPathDirectory -FixturePath
        Assert ($null -eq [NagiUninstallTestEnvironment]::UserPath -and [NagiUninstallTestEnvironment]::Writes -eq 0) "Absent User PATH was rewritten for $kind root"
        Assert ($env:Path -ceq 'keep-entry') "Unowned process PATH changed for $kind root"
    }

    Assert ([Environment]::GetEnvironmentVariable('Path', 'User') -ceq $previousUserPath) 'Actual User PATH changed'
    Assert ([Net.ServicePointManager]::SecurityProtocol -eq $previousTls) 'Uninstaller did not restore TLS settings'
    Assert ($fixture.Downloads -gt 0) 'Distribution verification did not use the local download fixture'
    Write-Host 'PowerShell uninstaller: preview, repeat, legacy, complete trees, protected files, foreign links, failed verification, lock, root junction, and isolated PATH tests passed.'
} finally {
    $env:Path = $previousPath
    [Net.ServicePointManager]::SecurityProtocol = $previousTls
    # Remove only junctions made by this test before any recursive fixture cleanup.
    foreach ($junction in $ownedJunctions) {
        $item = Get-Item -LiteralPath $junction -Force -ErrorAction SilentlyContinue
        if ($item -and ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { [IO.Directory]::Delete($junction) }
    }
    if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Recurse -Force }
}
