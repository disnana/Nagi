param(
    [Parameter(Mandatory=$true)][string]$Source,
    [string]$RustFile = '',
    [string[]]$RustDependencies = @(),
    [switch]$Offline
)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskSaved = @{}
foreach ($taskName in @('CARGO_NET_OFFLINE', 'RUSTFLAGS', 'NAGI_NATIVE_TARGET_DIR')) {
    $taskSaved[$taskName] = [Environment]::GetEnvironmentVariable($taskName, 'Process')
}
Push-Location -LiteralPath $taskRoot
try {
    if ($Offline) { $env:CARGO_NET_OFFLINE = 'true' }
    cargo build --release --locked -p nagic
    if ($LASTEXITCODE -ne 0) { throw 'Compiler build failed.' }
    $env:RUSTFLAGS = ($taskSaved['RUSTFLAGS'] + ' -C target-feature=+crt-static').Trim()
    $env:NAGI_NATIVE_TARGET_DIR = Join-Path $taskRoot 'build/windows-demo-native'
    $taskArgs = @('build', $Source)
    if ($RustFile) { $taskArgs += @('--rust', $RustFile) }
    foreach ($taskDep in $RustDependencies) { $taskArgs += @('--rust-dep', $taskDep) }
    & './target/release/nagic.exe' @taskArgs
    if ($LASTEXITCODE -ne 0) { throw 'Demo build failed.' }
    $taskPackage = 'nagi-' + [IO.Path]::GetFileNameWithoutExtension($Source).Replace('_', '-')
    $taskOutputDir = Join-Path $taskRoot 'build/distribution'
    New-Item -ItemType Directory -Path $taskOutputDir -Force | Out-Null
    $taskExe = Join-Path $taskOutputDir ($taskPackage + '.exe')
    Copy-Item -LiteralPath (Join-Path $env:NAGI_NATIVE_TARGET_DIR ('release/' + $taskPackage + '.exe')) -Destination $taskExe
    Write-Output "EXE: $taskExe"
    Write-Output ("Size: {0} bytes" -f (Get-Item -LiteralPath $taskExe).Length)
    Write-Output ("SHA256: {0}" -f (Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash)
}
finally {
    foreach ($taskName in $taskSaved.Keys) { [Environment]::SetEnvironmentVariable($taskName, $taskSaved[$taskName], 'Process') }
    Pop-Location
}
