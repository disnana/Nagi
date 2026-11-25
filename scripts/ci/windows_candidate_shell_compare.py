"""Compare native and PowerShell command capture on one installed Windows candidate."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

MARKER = '''    $result = & nagic run --project (Join-Path $project 'nagi.toml')
    if ($LASTEXITCODE -ne 0 -or @($result)[-1] -ne '42') { throw 'Installed compiler could not run an external project' }'''
HOOK = '''    & python $env:NAGI_CI_SHELL_COMPARE --probe (Join-Path $current 'nagic.exe') (Join-Path $project 'nagi.toml')
    if ($LASTEXITCODE -ne 0) { throw 'Native installed CLI comparison failed' }
    return'''
SHELL_PROBE = r"""
param([string]$Compiler, [string]$Project, [string]$Out, [string]$Mode)
$result = [ordered]@{ shell = $PSVersionTable.PSVersion.ToString(); error_id = '';
    category = ''; exit = $null; last_stdout = ''; exact_42 = $false }
$ErrorActionPreference = 'Stop'
try {
    if ($Mode -eq 'redirected') {
        & {
            $capture = & $Compiler run --project $Project
            $result.exit = $LASTEXITCODE
            $result.exact_42 = @( $capture ).Count -gt 0 -and @( $capture )[-1] -eq '42'
        } *> "$Out.redirected"
    } else {
        $capture = & $Compiler run --project $Project
        $result.exit = $LASTEXITCODE
        $result.exact_42 = @( $capture ).Count -gt 0 -and @( $capture )[-1] -eq '42'
        $capture
    }
} catch {
    $result.error_id = $_.FullyQualifiedErrorId
    $result.category = $_.CategoryInfo.Category.ToString()
    if ($_.Exception.Message -match 'warning:') { $result.last_stdout = 'native-warning' }
}
[IO.File]::WriteAllText($Out, ($result | ConvertTo-Json -Compress))
if ($result.error_id) { exit 1 }
exit $result.exit
"""


def stream_summary(data):
    return {
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "warning_marker": b"warning:" in data.lower(),
        "last_line_is_42": data.splitlines()[-1:] == [b"42"],
    }


def probe(compiler, project):
    with tempfile.TemporaryDirectory(prefix="nagi shell compare ") as scratch:
        folder = Path(scratch)
        source = folder / "capture.ps1"
        source.write_text(SHELL_PROBE, encoding="utf-8")
        results = []
        for shell in ("powershell.exe", "pwsh.exe"):
            for mode in ("redirected", "standard"):
                report = folder / f"{shell}-{mode}.json"
                run = subprocess.run(
                    [shell, "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
                     "-File", str(source), compiler, project, str(report), mode],
                    capture_output=True, timeout=300, check=False,
                )
                outcome = json.loads(report.read_text(encoding="utf-8-sig"))
                if not re.fullmatch(r"[A-Za-z0-9_,. -]{0,80}", outcome["error_id"]):
                    outcome["error_id"] = "interactive-native-error"
                redirected = report.with_suffix(".json.redirected")
                results.append({
                    "wrapper": shell, "mode": mode, "wrapper_exit": run.returncode,
                    "shell_observation": outcome, "stdout": stream_summary(run.stdout),
                    "stderr": stream_summary(run.stderr),
                    "powershell_redirected": stream_summary(redirected.read_bytes()) if redirected.exists() else None,
                })
        native = subprocess.run([compiler, "run", "--project", project], capture_output=True, timeout=300)
        result = {"shell_runs": results, "native": {
            "exit": native.returncode, "stdout": stream_summary(native.stdout), "stderr": stream_summary(native.stderr),
        }}
        print(json.dumps(result), flush=True)
        return int(native.returncode != 0 or native.stdout.splitlines()[-1:] != [b"42"])


def compare(checkout, archive):
    original = checkout / "scripts/releases/test_install.ps1"
    diagnostic = original.with_name("test_install.ci-compare.ps1")
    source = original.read_text(encoding="utf-8")
    if source.count(MARKER) != 1:
        raise RuntimeError("Installer probe anchor changed")
    with diagnostic.open("x", encoding="utf-8") as output:
        output.write(source.replace(MARKER, HOOK))
    try:
        env = dict(os.environ, NAGI_CI_SHELL_COMPARE=str(Path(__file__).resolve()))
        result = subprocess.run(
            ["pwsh.exe", "-NoProfile", "-NonInteractive", "-File", str(diagnostic),
             "-Archive", str(archive), "-Version", "0.3.0"],
            env=env, cwd=checkout, capture_output=True, timeout=1500,
        )
        for line in result.stdout.splitlines():
            if b'"shell_runs":' in line and b'"native":' in line:
                print(json.dumps(json.loads(line)), flush=True)
        print(f"Comparison exit: {result.returncode}", flush=True)
        return result.returncode
    finally:
        diagnostic.unlink()


if __name__ == "__main__":
    if sys.argv[1] == "--probe":
        raise SystemExit(probe(*sys.argv[2:]))
    raise SystemExit(compare(*(Path(arg).resolve() for arg in sys.argv[1:])))
