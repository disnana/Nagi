# Nagi

[日本語](README.md)

Nagi support for IntelliJ IDEA and PyCharm. Version 0.1.4 is being prepared for publication, preserving plugin ID `com.disnana.nagi` and display name **Nagi**. Check GitHub Releases and the Marketplace listing for available versions. [Install the compiler separately](https://nagi.disnana.com/en/docs/getting-started/).

The plugin version is managed in `build.gradle.kts`. The published 0.1.2 release includes a common ZIP for both IDEs; check Releases for newer available assets. For Marketplace installs, check the existing listing's Versions view for available versions and compatible IDE builds. Install it directly from [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/34891-nagi) in a compatible IDE, or download one common ZIP for both IDEs from [GitHub Releases](https://github.com/disnana/Nagi/releases).

## In development: named imports

The 0.2.1 compiler/plugin candidates add neighboring-file `import models` and `from models import Item`, import-statement completion, and original-source navigation. Published 0.2.0 and plugin 0.1.3 do not include this change. See the [editor guide](../../docs/en/editor.md#import-assistance-in-development) for candidate scope, background updates, external notification latency, and pending GUI verification.

## 0.1.4: hover and Quick Documentation (preparing publication)

The plugin candidate shows compiler-provided function signatures, parameters and return types, class fields, and variable types at exact compiler locations through mouse-hover documentation and Quick Documentation. Class construction uses named field arguments; an empty compiler parameter list is not presented as a zero-argument constructor. Parameter Info also shows a return type when the compiler supplies it.

Background analysis keeps its existing typing debounce. Colors for untouched names remain visible while analysis is pending; edits touching a name or changes to dependencies, settings or project trust discard them. Only visual styles are retained; old types, documentation, diagnostics and navigation are never reused.

Diagnostic hover text presents known CLI framing and token names more readably while keeping the original diagnostic description. Unknown text, notes and hints remain available. HTML and entity-like text are displayed literally, and meaningful type syntax such as `list[str?]` is preserved.

Documentation requires a trusted project, a matching assistance compiler and a current source snapshot. The same initial documentation request waits for background analysis. Edits or dependency, settings and trust changes cancel a pending request. Mouse documentation requires the IDE's hover documentation setting; the Quick Documentation action uses the same facts. Unsaved edits or dependency/compiler changes invalidate the old result. Missing compiler facts produce no inferred types or documentation. No parameter documentation is invented for builtins such as `print` when this protocol supplies no signature. This is partial editor analysis, not a full build check. These changes are unreleased; GUI acceptance is tracked separately from headless tests. See the [design and validation boundaries](../../docs/internal/jetbrains-documentation-plan.md).

## Features in 0.1.2

- Syntax highlighting for High (`.nagi`) and Low (`.low`).
- Line comments, matching and paired brackets, and block folding.
- Indentation on Enter: High definitions such as `def main():` and `case`, Low braces, and multiline parenthesized expressions.
- **Nagi: Check** and **Nagi: Run**. The nearest `nagi.toml` selects a project; otherwise the selected source file is used.
- A run button beside top-level `def main()`, `async def main()`, and Low `fn main()` declarations.
- Click compiler source locations in the Run console to open the file.

These features describe the published 0.1.2 release. It does not support whole-file formatting, semantic completion, go to definition, or automatic type checks. Type checking invokes the Nagi compiler manually.

The 0.1.3 implementation passed tests and Plugin Verifier on all four IDEA/PyCharm stable/EAP paths in [main CI 39422234923](https://github.com/disnana/Nagi/actions/runs/39422234923). Each EAP IDE retains three deprecated-API and one experimental-API Verifier warnings; this was not a warning-free result. Headless checks do not establish GUI acceptance. The user has accepted the earlier GUI checks; this update changes listing text and compiler guidance without changing IDE feature control flow. IME, the full EditorConfig modifier and live configuration reload remain unconfirmed.

## Editing improvements in 0.1.4 (preparing publication)

In High, consecutive Enter presses on an automatically indented blank line leave one block at a time. After typing, Backspace, paste or undo, Enter respects the indentation chosen by that operation. This exit gesture does not leave delimiter continuations or Low brace blocks.

Typing the colon of `else:` or `case ...:` aligns the High header with its known owner, preserving relative indentation in multiline case headers. A standalone closing-delimiter line aligns with its opening line: `)` and `]` in High, and `)`, `]` and `}` in Low. The new corrections require Smart Indent, a single caret and no selection replacement. Branch and closing-delimiter corrections require a known complete prefix within 8192 characters, 96 lines and 4096 tokens; ordinary input is preserved beyond those limits or when context is uncertain.

No Python plugin dependency is added. This does not establish full parity with PyCharm Python. The [internal comparison and implementation plan](../../docs/internal/jetbrains-python-editing-plan.md) records the researched features and remaining work. Full formatting, Rename and automatic imports are not included in this addition.

The user confirmed that the hover/color candidate works, and version 0.1.4 is being prepared for publication with public Nagi 0.2.0. Final version-specific CI and all four IDE test/Verifier paths must pass before distribution. GitHub publication and Marketplace submission/approval are separate. Candidate acceptance does not establish exhaustive GUI coverage.

## 0.1.3

This section describes 0.1.3 features and their limits, separately from 0.1.2. Install only a plugin ZIP listed in GitHub Releases or a compatible version offered by Marketplace. GitHub and Marketplace publication are separate; CI validation artifacts are not formal releases.

### Compiler compatibility

| Plugin | Compiler | Supported behavior |
|---|---|---|
| 0.1.4 (preparing publication) | Public Nagi 0.2.0 | Inherits 0.1.3 and improves documentation, initial-analysis waits, color retention and editing assistance |
| Published 0.1.2 | Public Nagi 0.2.0 / 0.1.11 | Manual Check/Run and basic editing; no semantic assistance, standard Run Configuration, or generation cleanup in 0.1.2 |
| 0.1.3 | Public Nagi 0.2.0 | High/Low semantic assistance, standard Run/Stop, and generation cleanup for IDE-managed runs |
| 0.1.3 | Public Nagi 0.1.11 | Ordinary Check/Run and normal execution through the standard Run Configuration; no assist protocol or generation-retention mode |
| 0.1.2 / 0.1.3 | No compiler | Syntax highlighting and compiler-independent basic editing; Check/Run and semantic assistance need a compiler |

[Install public compiler 0.2.0](https://nagi.disnana.com/en/docs/getting-started/) and confirm it with `nagic --version`. Semantic assistance in 0.1.3 no longer requires building a compiler from the same PR.

Version 0.1.3 updates High/Low editing support: IDE-standard bracket and quote pairing settings, bounded Enter indentation using the effective style width and spaces, and explicit function live templates for `def`, `async def`, and Low `fn` with editable placeholders. Version 0.1.3 connects Nagi indentation to the IDE’s standard indent-step Backspace action when smart Backspace is enabled, while preserving the OFF setting. At the start of a line, `d`/`de` in High and `f` in Low add the lexical `def`/`fn` keyword to normal completion when a top-level context can be confirmed. This keyword is suppressed beyond the first 8 KiB of a file or when an unclosed delimiter, string, or other uncertain context prevents that confirmation. Accepting the keyword inserts only its text; the existing live templates still require explicit expansion. Normal completion acceptance inserts only the function name; accepting with `(` assists the call parentheses. It also adds compiler-backed completion, structured diagnostics, definition navigation when the compiler provides an exact target, and a standard IDE Nagi Run Configuration. Unsaved buffers of previously saved local `.nagi` and `.low` files are included in analysis snapshots; new unsaved files are not supported. The plugin discards responses when the current source, open buffers, or compiler settings change.

Function parameter information uses the IDE's standard Parameter Info action (usually Ctrl+P). When the compiler resolves the function in the current source snapshot, the hint shows its signature and current argument. Cold analysis shares the existing debounced background worker; the compiler never starts on the EDT. Stale snapshots and moved carets do not reopen hints. Automatic hints respect IDE settings; an explicit action still works when automatic hints are off. The IDE does not infer signatures for unresolved functions or members.

High/Low syntax highlighting and lexical `def`/`fn` candidates in confirmed top-level contexts do not require a compiler. Variable/member completion, diagnostics, definition navigation, Parameter Info, and semantic coloring use public compiler 0.2.0. Analysis is partial editor analysis, not proof that a full build succeeds, and diagnostics are limited to the first compiler error returned. In a project, the source file must be in the entry's import or native graph. Candidates are omitted when the compiler cannot validate a read at that context; a suggestion does not guarantee that a later assignment, move, or call use is valid. Navigation is unavailable for references without an exact source span. Semantic assistance is blocked until changes to `nagi.toml` are saved.

The standard Run Configuration can be created from an existing local `.nagi` or `.low` file and uses the IDE's normal Run/Stop controls. It runs the nearest project's `nagi.toml` entry when present, saves open files first, and refuses to start in an untrusted project. Run preparation waits for pending saves without blocking the editor and can be canceled from the IDE's background task. If an open file has unsaved changes when preparation finishes, save and run again. Normal execution works with compiler 0.1.11. These 0.1.3 features are not part of the published 0.1.2 release. Compiler 0.2.0 is public; check Releases or Marketplace for available plugin versions.

Version 0.1.3 sets `NAGI_RUN_RETENTION=latest` only for IDE-managed Nagi runs and uses the generation-retention mode in public compiler 0.2.0. Published compiler 0.1.11 supports normal Check/Run but does not implement this mode or automatically prune successful CLI generations. Ordinary CLI generations remain immutable; this cleanup does not reclaim the shared Cargo cache in `build/native-target/` or `NAGI_NATIVE_TARGET_DIR`. This retention mode protects a latest generation only after a recorded successful native run, active generations, generations needed by all apps' input snapshots, and the last-good generation after a failed run. Compile success alone is not run success; managed generations with unknown input metadata are retained. Every successful generation, including ordinary builds, records canonical namespace references and input file identities; these protect dependencies of a non-latest active run or another app. If an external hard-link input shares the identity of a run lease, that lease and generation are preserved too. A dependency can remain until one additional successful sweep after its owner is reclaimed; there is no strict generation-count or disk-capacity limit. Unknown files, links/reparse points, and failed staging are also preserved. A recovery journal outside the generation tree is checked again on a later plugin-managed run; unknown or oversized records are preserved while recovery continues for valid records. This is not a CLI cleanup command or a power-loss durability guarantee.

Explicit path crates are protected as inputs. Generations exported as inputs to another output directory are retained as immutable artifacts and are not automatically reclaimed; a lease with remaining hardlinks is retained too. Nagi does not discover arbitrary Rust include or build-script dependencies.

The temporary compiler 0.1.12 candidate was cancelled and no artifact was published. The public compiler for plugin 0.1.3 is 0.2.0.

## Installation

For **0.1.2**, download `nagi-jetbrains-0.1.2.zip` and its `.sha256` from the [GitHub release](https://github.com/disnana/Nagi/releases/tag/jetbrains-v0.1.2). Use the same ZIP in IDEA and PyCharm. The plugin preserves `com.disnana.nagi` and its existing Marketplace page. The repository owner handles Marketplace submission; GitHub publication and Marketplace approval/publication are separate.

The `jetbrains-v0.1.1` release contains `nagi-jetbrains-IC-0.1.1.zip` for IDEA and `nagi-jetbrains-PC-0.1.1.zip` for PyCharm, each with a matching `.sha256` file.

To install a GitHub Release ZIP, select it in **Settings → Plugins → ⚙ → Install Plugin from Disk**, then restart the IDE. A PR build is available as the `release-jetbrains` artifact only after all IDEA/PyCharm stable/EAP checks pass in **Actions → Nagi checks**. For a local build, use the ZIP in `build/distributions/`.

Set the compiler executable in **Settings → Languages & Frameworks → Nagi**. An empty value uses `nagic` from `PATH`. Relative paths resolve from the IDE project root.

Open a Nagi file and choose **Nagi: Check** or **Nagi: Run** in its context menu or the Tools menu. These actions save open files before execution. The compiler does not run in untrusted projects.

In 0.1.2, the button beside `main` invokes the same **Nagi: Run** action. It does not use the Python or other run configuration selected in the top toolbar. When a nearby `nagi.toml` exists, it runs that project's entry point. Version 0.1.3 also exposes this Nagi run through a standard IDE Run Configuration with normal Run/Stop controls.

Output appears in the Run window. Check has a 30-second timeout, configurable from 1 to 300 seconds. Run has no timeout, so servers can keep running; use Stop in the Run window to end the process. Generated files are stored under `nagi` in the IDE system directory.

Building and running Nagi applications also requires Rust/Cargo and the build tools for your OS.

## Build and verification

Use JDK 21 for stable SDK checks and the common ZIP candidate build. EAP checks use the JDK required by the resolved SDK. The current 2026.3 EAP requires JDK 25, so EAP verification needs JDK 25. The Gradle JVM and Java compiler toolchain follow the IDE SDK: 21 for stable and 25 for the current EAP. The plugin Java API and class-file target remain fixed at `options.release=21` even when verification runs on JDK 25.

```sh
cd editors/jetbrains-nagi
./gradlew test buildPlugin
```

On Windows, use `gradlew.bat`. The wrapper pins Gradle 9.4.0, IntelliJ Platform Gradle Plugin 2.19.0, and IntelliJ IDEA Community 2025.1.1 as the default SDK. The first build downloads the SDK and dependencies.

The same code can be checked against the PyCharm SDK:

```sh
./gradlew -PplatformType=PC -PplatformVersion=2025.1.1 test buildPlugin
```

Use `-PlocalPlatformPath=/path/to/ide` to build against a local IDE. `runIde` starts an isolated development IDE.

Tests cover scanning, folding, indentation, CLI argument boundaries, diagnostic locations, and real IntelliJ Platform editor fixtures for file types, Enter, comments, paired brackets, and save failures. A real process also tests cancellation during startup. Set `NAGI_TEST_COMPILER` to a `nagic` executable to check High, Low, and project commands with the compiler. JetBrains CI builds the compiler from the same commit and runs this integration test for both IDEA and PyCharm.

The minimum target for 0.1.2 is 2025.1.1: IDEA build `251.25410.109` and PyCharm build `251.25410.122` were verified. The initial 2025.1 builds (`251.23774`) do not expose the public API used by the trusted-project check; Plugin Verifier reports an unresolved method, so those builds are excluded. CI first builds one common ZIP candidate against stable IDEA on JDK 21. It runs IDE tests from the same source on four SDKs—stable 2025.1.1 and EAP for both IDEA and PyCharm—and passes the same candidate ZIP to each Plugin Verifier. Stable tests use the Java 21 compiler toolchain; EAP tests use the toolchain required by each resolved SDK (currently JDK 25). The minimum IDEA/PyCharm 2025.1.1 targets are also checked by Plugin Verifier. Only after all four IDE tests and verifiers pass does CI promote the original candidate ZIP to the distribution artifact. The two ZIPs attached to the published 0.1.1 release are historical product-specific assets, separate from the common 0.1.2 ZIP. Marketplace UI installation and full IDE interaction are not covered by these checks.

Run `./gradlew test buildPlugin verifyPlugin` with JDK 21 for stable compatibility checks. When an EAP SDK requires JDK 25, start Gradle with JDK 25 and pass `-PnagiJavaToolchainVersion=25` to the EAP test (the stable default is 21). `options.release=21` keeps the plugin's compile target and Java API surface at 21 on either JDK. No Plugin Verifier warnings are muted; API compatibility, deprecated, and experimental findings remain visible. Add `-PminimumPlatformVersion=2025.1.1` for IDEA/PyCharm to check the minimum target too. Use `-PplatformVersion=LATEST-EAP-SNAPSHOT` to check the latest EAP. With `-PlocalPlatformPath`, only that local SDK is verified.

Official references: [Plugin SDK](https://plugins.jetbrains.com/docs/intellij/developing-plugins.html), [Gradle Plugin](https://plugins.jetbrains.com/docs/intellij/tools-intellij-platform-gradle-plugin.html). Licensed under [MIT](LICENSE).

## Minimum IDE

The common 0.1.2 plugin targets IntelliJ IDEA 2025.1.1 build 251.25410.109 or later and PyCharm 2025.1.1 build 251.25410.122 or later. The initial 2025.1 builds do not resolve the public trust API used to reject commands in untrusted projects. Version 0.1.2 drops 2024.3 and those initial builds. Users on excluded IDE builds can keep the published 0.1.1; upgrade to the supported build before updating the plugin. The ID `com.disnana.nagi` and existing Marketplace page remain unchanged. This support range applies to the GitHub 0.1.2 release and is retained for 0.1.3.

Definition navigation to virtual standard-library sources is not supported. In 0.1.3, record fields are navigable when compiler 0.2.0 returns an exact declaration location; references without one remain unsupported.
