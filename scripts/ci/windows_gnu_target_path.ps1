param(
    [Parameter(Mandatory = $true)]
    [string] $CurrentCompiler,

    [Parameter(Mandatory = $true)]
    [string] $EvidenceDirectory,

    [string] $BaselineVersion = '0.2.2',

    [string] $BaselineArchiveSha256 = 'eed9424998dcbca1bc148329626adb906e7f6b7ab5e4b85a227f33f6f5d76bf9'
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

$CurrentCompiler = (Resolve-Path -LiteralPath $CurrentCompiler).Path
$EvidenceDirectory = [IO.Path]::GetFullPath($EvidenceDirectory)
New-Item -ItemType Directory -Force -Path $EvidenceDirectory | Out-Null

function Remove-TreeIfPresent {
    param([Parameter(Mandatory = $true)] [string] $Path)
    if (Test-Path -LiteralPath $Path) {
        Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop
    }
    if (Test-Path -LiteralPath $Path) {
        throw "Could not remove the previous comparison path: $Path"
    }
}

$fixtureRoot = Join-Path $env:RUNNER_TEMP 'nagi-windows-gnu-target-path'
Remove-TreeIfPresent $fixtureRoot
New-Item -ItemType Directory -Force -Path $fixtureRoot | Out-Null

$archiveName = "nagi-$BaselineVersion-windows-x86_64.zip"
$archive = Join-Path $fixtureRoot $archiveName
$archiveUrl = "https://github.com/disnana/Nagi/releases/download/nagi-v$BaselineVersion/$archiveName"
Invoke-WebRequest -Uri $archiveUrl -OutFile $archive
$archiveDigest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($archiveDigest -ne $BaselineArchiveSha256) {
    throw "Baseline archive digest mismatch: expected $BaselineArchiveSha256, got $archiveDigest"
}
$baselineRoot = Join-Path $fixtureRoot 'baseline'
Expand-Archive -LiteralPath $archive -DestinationPath $baselineRoot
$baselineCompiler = (Resolve-Path -LiteralPath (Join-Path $baselineRoot "nagi-$BaselineVersion-windows-x86_64/nagic.exe")).Path

$project = Join-Path $fixtureRoot 'project with spaces'
New-Item -ItemType Directory -Force -Path (Join-Path $project 'src') | Out-Null
@'
entry = "src/main.nagi"
'@ | Set-Content -LiteralPath (Join-Path $project 'nagi.toml') -Encoding utf8
@'
def main() -> i32:
    print("hello")
    return 0
'@ | Set-Content -LiteralPath (Join-Path $project 'src/main.nagi') -Encoding utf8

$canonicalSource = Join-Path $fixtureRoot 'canonical.rs'
@'
fn main() {
    let path = std::env::args_os().nth(1).expect("path");
    println!("{}", std::fs::canonicalize(path).expect("canonical path").display());
}
'@ | Set-Content -LiteralPath $canonicalSource -Encoding utf8
$canonicalHelper = Join-Path $fixtureRoot 'canonical.exe'
& rustc '+stable-x86_64-pc-windows-msvc' $canonicalSource '-o' $canonicalHelper
if ($LASTEXITCODE -ne 0) { throw 'Could not compile the canonical-path witness' }
$canonicalProject = (& $canonicalHelper $project | Out-String).Trim()
if (-not $canonicalProject.StartsWith('\\?\')) {
    throw "Windows canonicalization did not produce a verbatim path: $canonicalProject"
}

function Clear-NagiCase {
    Remove-TreeIfPresent (Join-Path $project '.nagi')
    Remove-TreeIfPresent (Join-Path $project 'build')
    Remove-TreeIfPresent (Join-Path $project 'native-target')
}

function Invoke-NagiCase {
    param(
        [Parameter(Mandatory = $true)] [string] $Name,
        [Parameter(Mandatory = $true)] [string] $Compiler,
        [Parameter(Mandatory = $true)] [ValidateSet('explicit', 'project')] [string] $Mode
    )

    Clear-NagiCase
    $arguments = @('run')
    if ($Mode -eq 'explicit') { $arguments += 'src/main.nagi' }
    $log = Join-Path $EvidenceDirectory "$Name.log"
    $savedToolchain = $env:RUSTUP_TOOLCHAIN
    $savedTarget = $env:CARGO_TARGET_DIR
    $savedNativeTarget = $env:NAGI_NATIVE_TARGET_DIR
    $savedRuntimeProfile = $env:NAGI_RUNTIME_PROFILE
    $env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnu'
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:NAGI_NATIVE_TARGET_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:NAGI_RUNTIME_PROFILE -ErrorAction SilentlyContinue
    Push-Location $project
    try {
        $started = Get-Date
        $output = @(& $Compiler @arguments 2>&1 | ForEach-Object { "$_" })
        $exitCode = $LASTEXITCODE
        $elapsed = ((Get-Date) - $started).TotalSeconds
    }
    finally {
        Pop-Location
        if ($null -eq $savedToolchain) { Remove-Item Env:RUSTUP_TOOLCHAIN -ErrorAction SilentlyContinue } else { $env:RUSTUP_TOOLCHAIN = $savedToolchain }
        if ($null -eq $savedTarget) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue } else { $env:CARGO_TARGET_DIR = $savedTarget }
        if ($null -eq $savedNativeTarget) { Remove-Item Env:NAGI_NATIVE_TARGET_DIR -ErrorAction SilentlyContinue } else { $env:NAGI_NATIVE_TARGET_DIR = $savedNativeTarget }
        if ($null -eq $savedRuntimeProfile) { Remove-Item Env:NAGI_RUNTIME_PROFILE -ErrorAction SilentlyContinue } else { $env:NAGI_RUNTIME_PROFILE = $savedRuntimeProfile }
    }
    $output | Set-Content -LiteralPath $log -Encoding utf8
    $output | ForEach-Object { Write-Host "[$Name] $_" }
    $target = if ($Mode -eq 'explicit') { Join-Path $project 'native-target' } else { Join-Path $project 'build/native-target' }
    $result = [ordered]@{
        name = $Name
        compiler = $Compiler
        mode = $Mode
        exit_code = $exitCode
        elapsed_seconds = [Math]::Round($elapsed, 3)
        printed_hello = [bool]($output | Where-Object { $_ -eq 'hello' })
        target_created = Test-Path -LiteralPath $target
        log = [IO.Path]::GetFileName($log)
    }
    Clear-NagiCase
    return [PSCustomObject]$result
}

$rustcVersion = (& rustc '+stable-x86_64-pc-windows-gnu' '-vV' | Out-String).Trim()
$linkerVersion = (& x86_64-w64-mingw32-gcc '--version' | Out-String).Trim()
$baselineExplicit = Invoke-NagiCase -Name 'baseline-explicit' -Compiler $baselineCompiler -Mode explicit
if ($baselineExplicit.exit_code -ne 0 -or -not $baselineExplicit.printed_hello) {
    throw 'The explicit-source baseline control failed; this runner cannot distinguish issue #201'
}
$baselineProject = Invoke-NagiCase -Name 'baseline-project' -Compiler $baselineCompiler -Mode project
$currentProject = Invoke-NagiCase -Name 'current-project' -Compiler $CurrentCompiler -Mode project

$summary = [ordered]@{
    format = 'nagi-windows-gnu-target-path-v1'
    current_commit = $env:GITHUB_SHA
    baseline_version = $BaselineVersion
    baseline_archive_sha256 = $archiveDigest
    canonical_project_path = $canonicalProject
    rustc = $rustcVersion
    linker = $linkerVersion
    source_cache_shared = $true
    native_targets_fresh_per_case = $true
    baseline_project_reproduced = ($baselineProject.exit_code -ne 0)
    cases = @($baselineExplicit, $baselineProject, $currentProject)
}
$summaryPath = Join-Path $EvidenceDirectory 'summary.json'
$summary | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $summaryPath -Encoding utf8
$summary | ConvertTo-Json -Depth 6 | Write-Host

if ($currentProject.exit_code -ne 0 -or -not $currentProject.printed_hello) {
    throw 'The corrected project-mode compiler failed under Windows GNU/UCRT64'
}
if ($baselineProject.exit_code -eq 0) {
    Write-Warning 'Issue #201 did not reproduce on this hosted runner; the corrected case passed, but this is not proof of the reporter environment.'
}
