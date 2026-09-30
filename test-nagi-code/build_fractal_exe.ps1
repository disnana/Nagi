param([switch]$Offline)

$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskEnvNames = @('CARGO_NET_OFFLINE', 'RUSTFLAGS', 'NAGI_NATIVE_TARGET_DIR')
$taskSavedEnv = @{}
foreach ($taskName in $taskEnvNames) {
    $taskSavedEnv[$taskName] = [Environment]::GetEnvironmentVariable($taskName, 'Process')
}

Push-Location -LiteralPath $taskRoot
try {
    if ($Offline) { $env:CARGO_NET_OFFLINE = 'true' }
    cargo build --release --locked -p nagic
    if ($LASTEXITCODE -ne 0) { throw 'Compiler build failed.' }

    # Include the MSVC runtime so the delivered exe needs no VCRUNTIME140.dll.
    $env:RUSTFLAGS = ($taskSavedEnv['RUSTFLAGS'] + ' -C target-feature=+crt-static').Trim()
    $env:NAGI_NATIVE_TARGET_DIR = Join-Path $taskRoot 'build/fractal-native'
    & './target/release/nagic.exe' build test-nagi-code/fractal.nagi --out build/fractal
    if ($LASTEXITCODE -ne 0) { throw 'Fractal build failed.' }

    $taskOutput = Join-Path $taskRoot 'build/distribution'
    New-Item -ItemType Directory -Path $taskOutput -Force | Out-Null
    $taskExe = Join-Path $taskOutput 'nagi-fractal.exe'
    Copy-Item -LiteralPath (Join-Path $env:NAGI_NATIVE_TARGET_DIR 'release/nagi-fractal.exe') -Destination $taskExe
    Write-Output "EXE: $taskExe"
    Write-Output ("Size: {0} bytes" -f (Get-Item -LiteralPath $taskExe).Length)
    Write-Output ("SHA256: {0}" -f (Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash)
}
finally {
    foreach ($taskName in $taskEnvNames) {
        [Environment]::SetEnvironmentVariable($taskName, $taskSavedEnv[$taskName], 'Process')
    }
    Pop-Location
}
