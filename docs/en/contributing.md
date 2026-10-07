# Make your first contribution to Nagi

[Docs contents](README.md) · [Reproduce and run a bug](first-app.md) · [Repository contribution policy](../../CONTRIBUTING.en.md)

This walkthrough is for a first change to Nagi's compiler, runtime, standard libraries, or Docs. It follows a small compiler fix from the existing behavior to a pull request. Discuss syntax, types, ownership, failures, async behavior, or resource shutdown changes in an issue before implementing them.

## 1. Prepare the environment and branch

Install Git, stable Rust/Cargo, rustfmt, Clippy, and C build tools. See [setup](getting-started.md) for platform-specific toolchains. Create a fork of Nagi on GitHub, replace `YOUR_GITHUB_NAME` with your account, then clone it. From the repository root, create a branch from the latest upstream `main`:

```sh
git clone https://github.com/YOUR_GITHUB_NAME/Nagi.git
cd Nagi
git remote add upstream https://github.com/disnana/Nagi.git
git fetch upstream
git switch -c test/nullable-default upstream/main
git remote -v
cargo --version
rustc --version
cargo build --locked -p nagic
```

`git remote -v` should show your fork as the writable `origin` and the upstream repository as the read-only `upstream`. If you have permission to push directly to the main repository, you can use it as `origin` instead.

The development compiler can be run as `target/debug/nagic` (`target\debug\nagic.exe` on Windows). Confirm it starts:

```sh
./target/debug/nagic --version
```

Expected output is `nagic 0.1.11`. If Cargo is missing, check the Rust installation and PATH, then open a new terminal. C compiler or linker errors come from the build environment; see [platform build tools](getting-started.md#tools-for-building-applications) instead of treating them as Nagi type-check failures.

In Windows PowerShell, start the executable with `& .\target\debug\nagic.exe --version`. The remaining `./target/debug/nagic` commands refer to this development build.

## 2. Find the layer and its checks

Use the repository root as the current directory. Read [compiler internals](compiler-internals.md) and the [internal compiler pipeline](../internal/compiler-pipeline.md). If public behavior may change, also read the [language and runtime contracts](../internal/language-invariants.md).

| Location | Main responsibility |
|---|---|
| `compiler/src/lexer.rs`, `parser.rs` | Tokenization and High/Low parsing |
| `compiler/src/check.rs`, `view_flow.rs` | Type, ownership, borrow, and async checks |
| `compiler/src/emit.rs` | High-to-Low and checked-program-to-Rust generation, plus CLI |
| `runtime/src/` | Runtime APIs such as HTTP, SQLite, Tasks, and Actors |
| `compiler/tests/`, `tests/conformance/` | Unit/integration tests and High/Low regression cases |
| `docs/`, `docs/en/`, `website/` | Japanese/English Docs and the static site |

The [code map guide](code-map.md) describes `nagic map`, which diagrams Nagi applications. It does not diagram this Rust repository. Use `rg` (ripgrep), or Git's built-in `git grep` for tracked files, to find source files. Then follow the compiler documents above to trace parser, checker, and code generation responsibilities.

For example, search for uses of `parse_i64` and its compiler registration from the root:

```sh
rg -n 'parse_i64' compiler/src runtime/src tests/conformance compiler/tests
```

The results include its call sites and registration code, such as `compiler/src/stdlib.rs`. If the expected file is absent, shorten the search term and inspect candidates with `rg --files compiler/src runtime/src compiler/tests`. Without ripgrep, use `git grep -n 'parse_i64' -- compiler/src runtime/src tests/conformance compiler/tests` instead.

## 3. Reproduce the problem with a small example

Save an issue's reproduction as a separate file and record which Nagi check fails. From the repository root, check the existing tutorial program:

```sh
./target/debug/nagic check examples/tutorial/first_app.nagi
```

Success exits with code 0 and no Nagi diagnostics. If the change affects compiler/runtime behavior, also run the program with input:

```sh
printf '1000\n' | ./target/debug/nagic run examples/tutorial/first_app.nagi
```

In Windows PowerShell, use, for example, `"1000" | & .\target\debug\nagic.exe run examples\tutorial\first_app.nagi`. The expected output is in the [CLI input table](first-app.md#3-try-boundary-values-and-fix-a-mistake).

Keep the failure stage clear. `check` runs the Nagi parser/checker; `build` also runs Rust generation, Cargo, and rustc; `run` then executes the app. A failure in build tools or an external crate is not by itself evidence of a Nagi syntax or ownership bug. Conversely, if Nagi accepts code and generated Rust rejects a type, move, or lifetime problem Nagi can detect, treat it as a compiler bug.

## 4. Make a small fix and add regression coverage

Read the implementation and what the existing tests promise before editing. Parser or checker changes need a valid case and a rejected case, High/Low coverage, and the expected diagnostic file and line. Code generation and ownership changes need actual Rust build/run observations. Runtime changes should cover the affected boundaries among normal completion, failure, panic, cancellation, and resource shutdown using the existing harness.

Check whether an existing suite can cover the change before adding another test:

```sh
cargo test --locked -p nagic --test conformance
cargo test --locked -p nagic --test explicit_moves
cargo test --locked -p nagi-runtime --lib
```

The first two run the compiler regression corpus and explicit-move High/Low/Rust checks; the third runs runtime library tests. Choose targets that match the changed code. If a named `--test` binary does not exist, match the filename in `compiler/tests/` with `cargo test --locked -p nagic --test <file-name>`. See the [compiler test guide](../internal/compiler-testing.md) for test stages, corpus format, and failure triage.

For changed Rust code, run `cargo fmt --all -- --check` and the relevant tests. Compiler/runtime changes also require Clippy and `cargo test --locked`. For Docs-only changes, check matching Japanese and English pages, links, and any runnable example you changed; the full Rust suite is unnecessary.

### Small example: add a nullable conformance fixture

This example adds a regression fixture for `Some` and `None` without changing the implementation. From the repository root, create `tests/conformance/nullable_default.nagi`:

```nagi
def number_or_zero(value: i64?) -> i64:
    match value:
        case Some(number):
            return number
        case None:
            return 0
```

Add this case to the array in `tests/conformance/corpus.json`. Separate it from adjacent cases with a comma:

```json
{
  "name": "nullable_default",
  "source": "nullable_default.nagi",
  "high": true,
  "expected": "run-pass",
  "oracle": "assert_eq!(number_or_zero(Some(42i64)), 42i64); assert_eq!(number_or_zero(None), 0i64);",
  "diagnostic": "",
  "line": 0
}
```

A positive case with `high: true` checks High and its saved Low form; the `oracle` compares both native results. Check the source, then run the corpus test. Use the repository root as the current directory:

```sh
./target/debug/nagic check tests/conformance/nullable_default.nagi
cargo test --locked -p nagic --test conformance corpus_and_bounded_generated_contracts_reach_native_execution -- --exact
```

On success, the check prints `checked tests/conformance/nullable_default.nagi`. The named test reports `ok` and a `1 passed` summary. It runs the full registered corpus plus the default bounded generated cases, not only the new fixture. If it fails, inspect the reported case, stage, source line, fixture, and `source`/`oracle` entries. Confirm expected behavior against the contract and an independent result before changing an oracle to match output.

For site Docs, use Python 3.12+ with the dependencies from `website/requirements.txt`, then run this from the repository root:

```sh
python website/build.py --base-path / --out build/website-preview
```

On success, Japanese and English HTML is generated under `build/website-preview/`, and internal links and heading anchors are checked. For `ModuleNotFoundError`, use the virtual environment in [website setup](../../website/README.en.md#local-preview). If a link check fails, correct the Markdown source link or anchor shown in the error and run the build again.

## 5. Review the diff

From the repository root, stage only the two intended files and check for unrelated edits or generated files:

```sh
git add tests/conformance/corpus.json tests/conformance/nullable_default.nagi
git diff --cached --check
git status --short
git diff --cached -- tests/conformance/corpus.json tests/conformance/nullable_default.nagi
```

There should be no whitespace errors, and the diff should contain only the corpus update and new fixture. If other files appear, unstage them before committing and find out why they changed.

## 6. Commit and open a PR from your fork

After reviewing the staged diff, commit and push the branch from the repository root:

```sh
git commit -m "Add nullable conformance case"
git status --short
git push -u origin test/nullable-default
```

After the commit, an empty `git status --short` means there are no uncommitted changes. A successful push creates `test/nullable-default` on `origin`. On GitHub, open **Pull requests → New pull request** from your fork. Set the base repository to `disnana/Nagi`, the base branch to `main`, the compare repository to your fork, and the compare branch to `test/nullable-default`. Review the diff, then write a title and description with the problem, resulting behavior, test command and result, and any unverified limits before creating the PR.

Always target the latest `main`. If the branch is missing on GitHub, check the selected remote and branch name. See the root [CONTRIBUTING guide](../../CONTRIBUTING.en.md) for the full policy.

### When a step fails

| Symptom | What to check next |
|---|---|
| `nagic` does not show your latest change | Run `./target/debug/nagic` instead of the released compiler on PATH; rebuild if needed |
| `check` passes but `build` fails | Separate generated Rust/Cargo diagnostics from toolchain setup; see [compiler internals](compiler-internals.md) |
| A test fails | Check the first failing stage, expected/actual result, and source line. Do not weaken the contract to make it pass |
| Site build reports an import error | Activate the website Python venv and run `pip install -r website/requirements.txt` |
| You cannot find a likely source file | Start from the issue reproduction, search call sites with `rg` or `git grep`, and read existing tests and contracts. Discuss contract changes first |
