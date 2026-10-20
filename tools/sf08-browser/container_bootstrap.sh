#!/usr/bin/env bash
set -euo pipefail
# This script only exists for the explicitly disposable CI container.
python3 /input/tools/sf08-browser/provision.py --check
export PYTHONDONTWRITEBYTECODE=1
export PATH="/opt/rust/bin:$PATH"
exec python3 -B /input/tools/sf08-browser/ci_entrypoint.py --checkout /input
