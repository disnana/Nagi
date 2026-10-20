"""Canonical inverse for the two whole four-platform regression steps.

Call this before source-guard dedup and the unchanged historical inverses.
This accepts exactly the reviewed added job and exact dependency changes.
"""
import re

REGRESSION_JOB = '  nagi-regression:\n    needs: [changes, release-plan]\n    if: >-\n      needs.changes.result == \'success\' && needs.release-plan.result == \'success\' &&\n      needs.release-plan.outputs.package_nagi == \'true\'\n    strategy:\n      fail-fast: false\n      matrix:\n        include:\n          - os: ubuntu-latest\n            platform: linux-x86_64\n            executable: nagic\n            rust_host: x86_64-unknown-linux-gnu\n          - os: windows-latest\n            platform: windows-x86_64\n            executable: nagic.exe\n            rust_host: x86_64-pc-windows-msvc\n          - os: macos-15\n            platform: macos-arm64\n            executable: nagic\n            rust_host: aarch64-apple-darwin\n          - os: macos-15-intel\n            platform: macos-x86_64\n            executable: nagic\n            rust_host: x86_64-apple-darwin\n    runs-on: ${{ matrix.os }}\n    env:\n      NAGI_FAILURE_DIR: ${{ github.workspace }}/build/compiler-failures\n      NAGI_GOLDEN_CAPTURE_DIR: ${{ github.workspace }}/build/compiler-failures/goldens\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          ref: ${{ github.event.pull_request.head.sha || github.sha }}\n      - uses: dtolnay/rust-toolchain@stable\n        id: rust\n      - name: Cache the regression platform\'s Rust build\n        uses: actions/cache@v4\n        with:\n          path: |\n            ~/.cargo/registry\n            ~/.cargo/git\n            target\n            native-target\n          key: nagi-rust-v1-regression-${{ matrix.platform }}-${{ steps.rust.outputs.cachekey }}-${{ hashFiles(\'Cargo.lock\') }}-${{ hashFiles(\'Cargo.toml\', \'compiler/**\', \'runtime/**\') }}\n          restore-keys: |\n            nagi-rust-v1-package-${{ matrix.platform }}-${{ steps.rust.outputs.cachekey }}-${{ hashFiles(\'Cargo.lock\') }}-\n            nagi-rust-v1-package-${{ matrix.platform }}-${{ steps.rust.outputs.cachekey }}-\n      - uses: actions/setup-python@v5\n        with:\n          python-version: \'3.12\'\n      - uses: actions/setup-node@v4\n        with:\n          node-version: \'22\'\n      - name: Verify the compiler host architecture\n        env:\n          RELEASE_RUST_HOST: ${{ matrix.rust_host }}\n        shell: bash\n        run: |\n          python - <<\'PY\'\n          import os, subprocess\n          lines = subprocess.check_output([\'rustc\', \'-vV\'], text=True).splitlines()\n          host = next(line.removeprefix(\'host: \') for line in lines if line.startswith(\'host: \'))\n          assert host == os.environ[\'RELEASE_RUST_HOST\'], host\n          PY\n      - name: Fetch locked runtime dependencies before offline tests\n        env:\n          RELEASE_RUST_HOST: ${{ matrix.rust_host }}\n        shell: bash\n        run: cargo fetch --locked --target "$RELEASE_RUST_HOST"\n      - run: cargo build --release --locked -p nagic\n      - name: Check CLI, installation, ownership, and diagnostics on the target platform\n        run: cargo test --locked -p nagic --lib --test cli --test installation --test codegen --test loop_ownership --test ownership_calls --test ownership_boundaries --test comparison_ownership --test owned_copy_codegen --test build_diagnostics --test rust_dependencies --test expression_contracts --test frontend_contracts --test source_read_retention --test literal_contracts --test http_route_conflicts --test symbols --test editor_assistance --test modules --test module_parser --test module_emission --test module_symbols --test enum_parser --test enum_modules --test enum_emission --test enum_patterns --test nullable_database --test nullable_roundtrip --test owned_database --test borrow_free_returns --test static_callback_views --test view_iteration --test view_parameter_rebinding --test sql_check --test typed_errors --test error_routes --test class_field_types --test resource_contract_characterization --test stdlib_imports --test option_match --test http_stdlib --test http_codegen --test actor_stdlib --test actor_codegen --test graph_types --test graph_relations --test graph_render --test graph_html --test map_cli --test low_multiline --test list_ownership_inference --test list_borrow_iteration --test result_stdlib\n      - name: Check generated output protection and native expression behavior\n        run: cargo test --locked -p nagic --test output_safety --test integer_arithmetic --test integer_zero_division --test constant_validation --test auth_boundaries --test view_branch_rebinding --test view_container_rebinding --test view_container_drop --test view_flow_foundation --test view_flow_completion --test explicit_moves --test task_handles --test conformance --test scope_runtime_contract --test copy_capabilities --test shared_field_moves --test asset_calls --test shared_target --test build_generations --test project\n      - name: Preserve regression compiler and application failure evidence\n        if: failure()\n        uses: actions/upload-artifact@v4\n        with:\n          name: compiler-failures-regression-${{ matrix.platform }}\n          path: build/compiler-failures/\n          if-no-files-found: ignore\n\n'

CLI_STEP = '      - name: Check CLI, installation, ownership, and diagnostics on the target platform\n        run: cargo test --locked -p nagic --lib --test cli --test installation --test codegen --test loop_ownership --test ownership_calls --test ownership_boundaries --test comparison_ownership --test owned_copy_codegen --test build_diagnostics --test rust_dependencies --test expression_contracts --test frontend_contracts --test source_read_retention --test literal_contracts --test http_route_conflicts --test symbols --test editor_assistance --test modules --test module_parser --test module_emission --test module_symbols --test enum_parser --test enum_modules --test enum_emission --test enum_patterns --test nullable_database --test nullable_roundtrip --test owned_database --test borrow_free_returns --test static_callback_views --test view_iteration --test view_parameter_rebinding --test sql_check --test typed_errors --test error_routes --test class_field_types --test resource_contract_characterization --test stdlib_imports --test option_match --test http_stdlib --test http_codegen --test actor_stdlib --test actor_codegen --test graph_types --test graph_relations --test graph_render --test graph_html --test map_cli --test low_multiline --test list_ownership_inference --test list_borrow_iteration --test result_stdlib\n'

GENERATED_STEP = '      - name: Check generated output protection and native expression behavior\n        run: cargo test --locked -p nagic --test output_safety --test integer_arithmetic --test integer_zero_division --test constant_validation --test auth_boundaries --test view_branch_rebinding --test view_container_rebinding --test view_container_drop --test view_flow_foundation --test view_flow_completion --test explicit_moves --test task_handles --test conformance --test scope_runtime_contract --test copy_capabilities --test shared_field_moves --test asset_calls --test shared_target --test build_generations --test project\n'

EDITOR_ANCHOR = '      - name: Check editor behavior on the target platform\n        shell: bash\n        run: node --test editors/vscode-nagi/test/*.test.js scripts/test_graph_html_viewport.mjs\n'

SF01_ANCHOR = '      - name: Check bounded SF01 diagnostic observer\n        env:\n          RUSTFLAGS: --cfg nagi_sf01_diagnostic --cfg nagi_sf01_diag_oracle --check-cfg=cfg(nagi_sf01_diagnostic) --check-cfg=cfg(nagi_sf01_diag_oracle)\n        run: cargo test --locked -p nagic --test security_sf01 sf01_diagnostic_observer_oracle -- --exact\n'

READY_OLD = '    needs: [changes, linux, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package]\n'

READY_NEW = '    needs: [changes, linux, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package, nagi-regression]\n'

PUBLISH_OLD = '    needs: [changes, linux, rust-1-88, sf08-browser, release-plan, jetbrains, vscode-package, nagi-package]\n'

PUBLISH_NEW = '    needs: [changes, linux, rust-1-88, sf08-browser, release-plan, jetbrains, vscode-package, nagi-package, nagi-regression]\n'

PUBLISH_CONDITION_OLD = "      (needs.nagi-package.result == 'success' || needs.nagi-package.result == 'skipped') &&\n"

PUBLISH_CONDITION_NEW = "      (needs.nagi-package.result == 'success' || needs.nagi-package.result == 'skipped') &&\n      ((needs.release-plan.outputs.package_nagi == 'true' && needs.nagi-regression.result == 'success') ||\n       (needs.release-plan.outputs.package_nagi == 'false' && needs.nagi-regression.result == 'skipped')) &&\n      (needs.release-plan.outputs.release_nagi != 'true' || needs.nagi-regression.result == 'success') &&\n"

def _replace_once(source, new, old, label):
    if source.count(new) != 1:
        raise ValueError(f"parallel regression {label} mismatch")
    return source.replace(new, old, 1)


def inverse_workflow(source):
    if len(re.findall(r"(?m)^  nagi-regression:$", source)) != 1:
        raise ValueError("parallel regression job count mismatch")
    source = _replace_once(source, REGRESSION_JOB, "", "job")
    match = re.search(r"(?ms)^  nagi-package:\n.*?(?=^  [a-zA-Z0-9_-]+:|\Z)", source)
    if match is None:
        raise ValueError("parallel regression package job missing")
    package = match.group()
    if CLI_STEP in source or GENERATED_STEP in source:
        raise ValueError("parallel regression moved step still present")
    package = _replace_once(package, EDITOR_ANCHOR, CLI_STEP + EDITOR_ANCHOR, "CLI anchor")
    package = _replace_once(package, SF01_ANCHOR, GENERATED_STEP + SF01_ANCHOR, "generated anchor")
    source = source[:match.start()] + package + source[match.end():]
    for new, old, label in (
        (READY_NEW, READY_OLD, "ready needs"),
        (PUBLISH_NEW, PUBLISH_OLD, "publish needs"),
        (PUBLISH_CONDITION_NEW, PUBLISH_CONDITION_OLD, "publish condition"),
    ):
        source = _replace_once(source, new, old, label)
    return source
