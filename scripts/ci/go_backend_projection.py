"""Remove only the reviewed Go-backend CI gate before historical projections."""

GO_JOB = r'''
  go-backend:
    name: Go backend (${{ matrix.platform }})
    needs: [changes, ci-plan]
    if: needs.changes.result == 'success' && needs.ci-plan.result == 'success' && needs.ci-plan.outputs.go_checks == 'true'
    runs-on: ${{ matrix.os }}
    timeout-minutes: 30
    strategy:
      fail-fast: false
      matrix: ${{ fromJSON(needs.ci-plan.outputs.matrix) }}
    env:
      CARGO_TERM_COLOR: never
      GOTOOLCHAIN: local
      NAGI_GO: go
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # ci-pin:v4
        with:
          persist-credentials: false # ci-isolation
      - uses: dtolnay/rust-toolchain@b725eda525362e7cf34af9a7243cac1e3f01a7ec # ci-pin:stable
      - uses: actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065 # ci-pin:v5
        with:
          python-version: '3.12'
      - uses: actions/setup-go@b7ad1dad31e06c5925ef5d2fc7ad053ef454303e # ci-pin:v7
        with:
          go-version: '1.27.3'
          cache: false
      - name: Verify the selected Go toolchain
        shell: bash
        run: |
          set -euo pipefail
          test "$(go env GOTOOLCHAIN)" = "local"
          go version
      - name: Run the real Go backend build and execution contract
        env:
          GOCACHE: ${{ runner.temp }}/nagi-go-build-cache
        shell: bash
        run: |
          set -euo pipefail
          mkdir -p build/ci-evidence/go-backend
          cargo test --locked -p nagic --test go_backend 2>&1 | tee "build/ci-evidence/go-backend/${{ matrix.platform }}.log"
      - name: Reject missing, skipped, or partial Go backend execution
        run: >-
          python -B scripts/ci/assert_go_backend_summary.py
          "build/ci-evidence/go-backend/${{ matrix.platform }}.log"
          --expected "${{ matrix.go_tests }}"
      - name: Preserve Go backend failure evidence
        if: failure()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: go-backend-failures-${{ matrix.platform }}
          path: build/ci-evidence/go-backend/
          if-no-files-found: ignore
'''

EDITS = (
    ("      go_checks: ${{ steps.changes.outputs.go_checks }}\n", ""),
    ("      go_checks: ${{ steps.level.outputs.go_checks }}\n", ""),
    ("          CI_GO: ${{ needs.changes.outputs.go_checks }}\n", ""),
    (GO_JOB, ""),
    (
        "    needs: [changes, ci-plan, critical, go-backend, linux, linux-workspace, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package, nagi-regression, windows-contracts]\n",
        "    needs: [changes, ci-plan, critical, linux, linux-workspace, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package, nagi-regression, windows-contracts]\n",
    ),
    (
        "    needs: [changes, ci-plan, critical, go-backend, linux, linux-workspace, rust-1-88, sf08-browser, release-plan, jetbrains, vscode-package, nagi-package, nagi-regression, windows-contracts, ready]\n",
        "    needs: [changes, ci-plan, critical, linux, linux-workspace, rust-1-88, sf08-browser, release-plan, jetbrains, vscode-package, nagi-package, nagi-regression, windows-contracts, ready]\n",
    ),
)


def inverse_workflow(source: str) -> str:
    for added, inherited in EDITS:
        if source.count(added) != 1:
            raise ValueError("Go backend CI inverse anchor mismatch")
        source = source.replace(added, inherited, 1)
    return source
