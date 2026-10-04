[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Nagi\versions'),
    [switch]$NoPath
)
$ErrorActionPreference = 'Stop'
$architecture = $env:PROCESSOR_ARCHITEW6432
if (-not $architecture) { $architecture = $env:PROCESSOR_ARCHITECTURE }
if ($env:OS -ne 'Windows_NT' -or $architecture -ne 'AMD64') {
    throw 'This uninstaller supports Windows x64. Use uninstall.sh on Linux/macOS.'
}

function Get-PlainDirectory([string]$Path) {
    $item = Get-Item -Force -LiteralPath $Path
    $root = $item
    while ($item) {
        if (-not ($item -is [IO.DirectoryInfo]) -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Directory contains a link or is not a directory; kept unchanged: $Path"
        }
        $item = $item.Parent
    }
    if ($root.FullName -eq [IO.Path]::GetPathRoot($root.FullName)) { return $root.FullName }
    return $root.FullName.TrimEnd('\', '/')
}

function Get-Distribution([string]$Release, [string]$Folder) {
    $name = "nagi-$Release-windows-x86_64"
    $asset = "$name.zip"
    $base = "https://github.com/disnana/Nagi/releases/download/nagi-v$Release"
    New-Item -ItemType Directory -Path $Folder -WhatIf:$false -Confirm:$false | Out-Null
    foreach ($file in @($asset, "$asset.sha256")) {
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$file" -OutFile (Join-Path $Folder $file)
    }
    $checksum = (Get-Content -Raw -LiteralPath (Join-Path $Folder "$asset.sha256")).Trim()
    if ($checksum -notmatch '^([0-9a-f]{64})  (\S+)$' -or $Matches[2] -ne $asset) { throw 'Invalid checksum file' }
    $expected = $Matches[1]
    $archive = Join-Path $Folder $asset
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) { throw 'SHA-256 mismatch' }
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
    Expand-Archive -LiteralPath $archive -DestinationPath $Folder -WhatIf:$false -Confirm:$false
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

function Test-AvailableFiles([string]$Root) {
    $handles = New-Object 'System.Collections.Generic.List[System.IDisposable]'
    try {
        # Snapshot rejects reparse points before this traversal. Do not stop
        # applications to release their files; preserve an in-use tree.
        foreach ($file in Get-ChildItem -Force -LiteralPath $Root -Recurse -File) {
            $handles.Add([IO.File]::Open($file.FullName, 'Open', 'Read', 'Delete'))
        }
    } finally { foreach ($handle in $handles) { $handle.Dispose() } }
}

function Get-CleanedPath($Value, $OwnedPaths) {
    if ($null -eq $Value) { return $null }
    $parts = New-Object 'System.Collections.Generic.List[string]'
    foreach ($part in ($Value -split ';')) {
        $normalized = $part.Trim().Trim('"').TrimEnd('\', '/').Replace('/', '\')
        $owned = $false
        foreach ($path in $OwnedPaths) {
            if ($normalized -ieq $path) { $owned = $true; break }
        }
        if (-not $owned) { $parts.Add($part) }
    }
    return ($parts -join ';')
}

$InstallDir = [IO.Path]::GetFullPath($InstallDir)
$rootItem = Get-Item -Force -LiteralPath $InstallDir -ErrorAction SilentlyContinue
if (-not $rootItem) { Write-Host "[OK] No installation directory: $InstallDir"; return }
$InstallDir = Get-PlainDirectory $InstallDir
$current = Join-Path $InstallDir 'current'
$lockPath = Join-Path $InstallDir '.install.lock'
$lock = $null
$work = Join-Path ([IO.Path]::GetTempPath()) ('nagi-uninstall-' + [guid]::NewGuid())
$previousTls = [Net.ServicePointManager]::SecurityProtocol
$ownedPaths = New-Object 'System.Collections.Generic.List[string]'
$verified = New-Object 'System.Collections.Generic.List[object]'
try {
    $lockItem = Get-Item -Force -LiteralPath $lockPath -ErrorAction SilentlyContinue
    if ($lockItem -and ($lockItem.PSIsContainer -or ($lockItem.Attributes -band [IO.FileAttributes]::ReparsePoint))) { throw 'Installer lock is not a plain file; kept unchanged' }
    try {
        # WhatIf leaves the installation root unchanged, including legacy
        # layouts that have never created the shared installer lock file.
        if (-not $WhatIfPreference -or $lockItem) {
            $mode = if ($WhatIfPreference) { 'Open' } else { 'OpenOrCreate' }
            $lock = [IO.File]::Open($lockPath, $mode, 'ReadWrite', 'None')
        }
    } catch { throw "Another installer/uninstaller is running, or the install directory is not writable: $InstallDir" }
    New-Item -ItemType Directory -Path $work -WhatIf:$false -Confirm:$false | Out-Null
    [Net.ServicePointManager]::SecurityProtocol = $previousTls -bor [Net.SecurityProtocolType]::Tls12
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $existing = Get-Item -Force -LiteralPath $current -ErrorAction SilentlyContinue
    $managedCurrent = $false
    if ($existing) {
        $managed = '^' + [regex]::Escape((Join-Path $InstallDir 'nagi-')) + '\d+\.\d+\.\d+-windows-x86_64$'
        $targets = @($existing.Target)
        if ($existing.LinkType -eq 'Junction' -and $targets.Count -eq 1 -and $targets[0] -match $managed) { $managedCurrent = $true }
        else { Write-Warning "Kept existing current path: $current" }
    }
    foreach ($old in Get-ChildItem -Force -LiteralPath $InstallDir -Directory) {
        if ($old.Name -notmatch '^nagi-(\d+\.\d+\.\d+)-windows-x86_64$') { continue }
        $version = $Matches[1]
        try {
            $snapshot = Get-InstallSnapshot $old.FullName
            $reference = Get-Distribution $version (Join-Path $work "release-$version")
            if ((Get-InstallSnapshot $reference) -cne $snapshot -or (Get-InstallSnapshot $old.FullName) -cne $snapshot) { throw 'Files were added or changed' }
            Test-AvailableFiles $old.FullName
            $verified.Add([pscustomobject]@{ Path = $old.FullName; Snapshot = $snapshot })
            $ownedPaths.Add($old.FullName)
        } catch { Write-Warning "Kept changed, busy or unverifiable installation: $($old.FullName) ($($_.Exception.Message))" }
    }
    if ($managedCurrent -and $PSCmdlet.ShouldProcess($current, 'Remove installer current junction')) {
        $check = Get-Item -Force -LiteralPath $current
        $checkTargets = @($check.Target)
        if ($check.LinkType -ne 'Junction' -or $checkTargets.Count -ne 1 -or $checkTargets[0] -ine $targets[0]) { throw 'Current junction changed during uninstall; kept unchanged' }
        # Delete the junction itself, never recursively traverse its target.
        [IO.Directory]::Delete($current)
        $ownedPaths.Add($current)
        Write-Host "Removed command junction: $current"
    }
    if (-not $WhatIfPreference) {
        $env:Path = Get-CleanedPath $env:Path $ownedPaths
        if (-not $NoPath) {
            $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
            $cleaned = Get-CleanedPath $userPath $ownedPaths
            if ($userPath -cne $cleaned -and $PSCmdlet.ShouldProcess('User PATH', 'Remove installer-owned Nagi entries')) {
                [Environment]::SetEnvironmentVariable('Path', $cleaned, 'User')
                if ([Environment]::GetEnvironmentVariable('Path', 'User') -cne $cleaned) { throw 'Could not save User PATH' }
            }
        }
    } else {
        if ($managedCurrent) { $ownedPaths.Add($current) }
        if (-not $NoPath -and $ownedPaths.Count) { [void]$PSCmdlet.ShouldProcess('User PATH', 'Remove installer-owned Nagi entries') }
    }
    foreach ($entry in $verified) {
        if ($PSCmdlet.ShouldProcess($entry.Path, 'Remove unchanged verified Nagi distribution')) {
            try {
                if ((Get-InstallSnapshot $entry.Path) -cne $entry.Snapshot) { throw 'Files changed during uninstall' }
                Test-AvailableFiles $entry.Path
                Remove-Item -LiteralPath $entry.Path -Recurse -Force -Confirm:$false
                Write-Host "Removed distribution: $($entry.Path)"
            } catch { Write-Warning "Kept installation: $($entry.Path) ($($_.Exception.Message))" }
        }
    }
    if ($WhatIfPreference) { Write-Host '[OK] Dry run complete; no installed files or PATH entries changed.' }
    else {
        Write-Host '[OK] Nagi uninstall complete. Any kept items are listed above.'
        Write-Host 'Restart your terminal and VS Code to refresh PATH.'
    }
} finally {
    [Net.ServicePointManager]::SecurityProtocol = $previousTls
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force -WhatIf:$false -Confirm:$false }
    if ($lock) { $lock.Dispose() }
}
