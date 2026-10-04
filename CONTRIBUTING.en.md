# Contributing to Nagi

[日本語](CONTRIBUTING.md)

Bug reports, code fixes, documentation, and translations are welcome. Issues and PRs may be written in Japanese or English. See the [Docs](docs/en/README.md) for the language and implementation scope, and the [roadmap](docs/en/roadmap.md) for planned work.

## How changes are evaluated

Nagi aims to support common backends in readable High, with access to Rust libraries where needed. See the [introduction](docs/en/introduction.md) for the current scope, and [DESIGN](DESIGN.en.md) for the rationale and open questions.

- Show a concrete task that is unsupported or difficult. Check whether existing APIs or Rust integration can solve it, and explain why new syntax or a dedicated implementation is needed.
- Discuss behavior and compatibility before implementing changes to types, ownership, failures, cancellation, or resource cleanup. The same standard applies to APIs wrapping Rust libraries.
- Keep claims within the evidence. Distinguish implementation, passing tests, and language guarantees. Different measurement conditions or test counts alone do not establish performance or safety.

## Report a bug or propose a change

Check existing [issues](https://github.com/disnana/Nagi/issues) and [PRs](https://github.com/disnana/Nagi/pulls) first. A useful bug report includes:

- The Nagi and editor extension versions or commit, and your OS.
- A minimal example and the commands needed to reproduce it.
- The expected result, actual result, and error messages.

Remove secrets and personal data from logs. Follow the [security policy](SECURITY.en.md) to report vulnerability details privately.

Discuss changes to language syntax or semantics, public APIs, new dependencies, and release or deployment processes in an issue before implementing them. Small fixes to typos or existing behavior can go straight to a PR.

## Open a PR

Create a branch from the latest `main` and target `main`. Keep each PR focused on one purpose. Leave out unrelated formatting and generated files.

Explain the problem, the resulting behavior, and the checks you ran with their results. State any checks you did not run and limitations that remain. For a bug fix, add a regression test where appropriate that fails before the fix and passes afterward. Performance claims need measurement conditions and raw results.

Keep documentation and examples consistent with the implementation and update matching Japanese and English pages. Distinguish changes on main from published features; put examples of unimplemented APIs on design pages. Run changed executable examples to verify them.

Lead with the operation and its result. Describe limitations through the conditions that trigger them and the resulting behavior. Omit repeated cautions and unsupported promotional claims.

During review, explain your changes in your own words and discuss the code and content respectfully.

## Check your work

Run commands from the repository root. Compiler and runtime development requires stable Rust/Cargo, rustfmt, Clippy, and a C build environment. See [setup](docs/en/getting-started.md) for platform instructions.

For Rust changes, run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

Run additional checks for the affected component. Python scripts use Python 3.12 or later; editor tests use Node.js 22.

| Changed component | Checks |
|---|---|
| Compiler or runtime | Build/run examples, plus HTTP integration tests when relevant. See the [README commands](README.en.md#reproduce-the-checks) |
| VS Code extension | `node --test editors/vscode-nagi/test/*.test.js` |
| JetBrains plugin | Follow the [plugin checks](editors/jetbrains-nagi/README.en.md#build-and-verification); distinguish real-compiler integration from IDE compatibility checks |
| Release scripts | `python -m unittest discover -s scripts/releases -p 'test_*.py'` |
| Published website or Docs | Build and check links using the [site instructions](website/README.md#手元で確認する) (Japanese) |

Documentation-only changes do not require the full Rust test suite. Check links, matching translations, and any runnable examples you changed. The [CI workflow](.github/workflows/ci.yml) contains the automated checks.

For changes to editor APIs or compiler invocation, also exercise the affected operations in the actual editor and report the product, version, and results.

Use `python scripts/verify_application_examples.py --compiler /path/to/nagic` to exercise practical examples. It currently covers seven projects: six High applications and their saved Low forms, plus one application written in Low, for thirteen check/build paths. It verifies input handling, HTTP, database persistence, and actor behavior. Reduce discovered bugs to small regression cases and include valid controls that should remain accepted.

## Using AI tools

AI tools are allowed. The person submitting a contribution remains fully responsible for its accuracy, verification, security, and the right to contribute it. This includes code, documentation, issues, PRs, and review comments. AI use does not excuse errors or missing verification.

- Read all generated output and understand why each change is needed and how it works before submitting it. Do not submit changes you cannot explain.
- Run the checks needed for your change and inspect their results. An AI tool's success claim or agreement between AI tools is not verification. Do not fabricate tests, logs, measurements, or references.
- Verify APIs, specifications, and quotations against the implementation or official sources. Check the provenance and licenses of generated code and text.
- Do not send secrets, code, or personal data to an AI service without permission to share it.

Edit generated content before posting it. Keep the facts and explanations that matter; remove repetition, unsupported claims, and irrelevant boilerplate. The same quality standards apply to contributions made without AI.

## License

Nagi uses the [MIT license](LICENSE). Submit your own changes under the same license. If you include third-party code or text, verify that you have the right to distribute it under compatible terms, and retain required copyright and license notices. These checks also apply to content generated or rewritten with AI.
