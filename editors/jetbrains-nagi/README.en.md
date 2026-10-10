# Nagi

[日本語](README.md)

Nagi support for IntelliJ IDEA and PyCharm. The next update candidate uses **Nagi** as its display name. The Marketplace listing keeps its current name until that update is published. [Install the compiler separately](https://nagi.disnana.com/en/docs/getting-started/).

The plugin version is managed in `build.gradle.kts`. The common ZIP on GitHub Releases is version 0.1.2. For Marketplace installs, check the existing listing's Versions view for available versions and compatible IDE builds. Install it directly from [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/34891-nagi) in a compatible IDE, or download one common ZIP for both IDEs from [GitHub Releases](https://github.com/disnana/Nagi/releases).

## Features in 0.1.2

- Syntax highlighting for High (`.nagi`) and Low (`.low`).
- Line comments, matching and paired brackets, and block folding.
- Indentation on Enter: High definitions such as `def main():` and `case`, Low braces, and multiline parenthesized expressions.
- **Nagi: Check** and **Nagi: Run**. The nearest `nagi.toml` selects a project; otherwise the selected source file is used.
- A run button beside top-level `def main()`, `async def main()`, and Low `fn main()` declarations.
- Click compiler source locations in the Run console to open the file.

These features describe the published 0.1.2 release. It does not support whole-file formatting, semantic completion, go to definition, or automatic type checks. Type checking invokes the Nagi compiler manually.

## 0.1.3 candidate (unreleased)

A separate unreleased candidate adds compiler-backed completion for High and Low, structured diagnostics, go to definition when the compiler provides an exact source target, and a standard IDE Nagi Run Configuration. Unsaved buffers of previously saved local `.nagi` and `.low` files are included in analysis snapshots; new unsaved files are not supported. The candidate discards responses when the current source, open buffers, or compiler settings change.

The plugin's High/Low syntax highlighting does not require a compiler, and published `nagic` 0.1.11 continues to support manual Check/Run and normal execution through the Run Configuration. Semantic assistance for completion, diagnostics, and definition navigation requires a **development `nagic` built from the same PR source commit**. Published 0.1.11 does not implement the assist protocol; set the matching compiler CI artifact to use these features. Analysis is partial editor analysis, not proof that a full build succeeds, and diagnostics are limited to the first compiler error returned. In a project, the source file must be in the entry's import or native graph. Candidates are omitted when the compiler cannot validate a read at that context; a suggestion does not guarantee that a later assignment, move, or call use is valid. Navigation is unavailable for references without an exact source span, including record-field declarations in the current compiler response. Semantic assistance is blocked until changes to `nagi.toml` are saved.

The standard Run Configuration can be created from an existing local `.nagi` or `.low` file and uses the IDE's normal Run/Stop controls. It runs the nearest project's `nagi.toml` entry when present, saves open files first, and refuses to start in an untrusted project. Normal execution works with compiler 0.1.11. These unreleased 0.1.3 candidate features are not part of the published 0.1.2 release. The candidate plugin and matching compiler for semantic assistance are PR CI artifacts, not GitHub or Marketplace releases.

The candidate is also testing a generation-retention mode that sets `NAGI_RUN_RETENTION=latest` only for IDE-managed Nagi runs, with a development compiler built from the same PR source. Published compiler 0.1.11 supports normal Check/Run but does not implement this mode or automatically prune successful CLI generations. Ordinary CLI generations remain immutable; this cleanup does not reclaim the shared Cargo cache in `build/native-target/` or `NAGI_NATIVE_TARGET_DIR`. Candidate retention protects a latest generation only after a recorded successful native run, active generations, generations needed by all apps' input snapshots, and the last-good generation after a failed run. Compile success alone is not run success; managed generations with unknown input metadata are retained. Every successful generation, including ordinary builds, records canonical namespace references and input file identities; these protect dependencies of a non-latest active run or another app. If an external hard-link input shares the identity of a run lease, that lease and generation are preserved too. A dependency can remain until one additional successful sweep after its owner is reclaimed; there is no strict generation-count or disk-capacity limit. Unknown files, links/reparse points, and failed staging are also preserved. A recovery journal outside the generation tree is checked again on a later plugin-managed run; unknown or oversized records are preserved while recovery continues for valid records. This feature is still under validation; it is not a CLI cleanup command or a power-loss durability guarantee.

Explicit path crates are protected as inputs. Generations exported as inputs to another output directory are retained as immutable artifacts and are not automatically reclaimed; a lease with remaining hardlinks is retained too. Nagi does not discover arbitrary Rust include or build-script dependencies.

The temporary compiler 0.1.12 candidate was cancelled and no artifact was published. JetBrains plugin 0.1.3 and the matching development compiler from the same PR source remain unreleased candidates under validation.

## Installation

For **0.1.2**, download `nagi-jetbrains-0.1.2.zip` and its `.sha256` from the [GitHub release](https://github.com/disnana/Nagi/releases/tag/jetbrains-v0.1.2). Use the same ZIP in IDEA and PyCharm. The plugin preserves `com.disnana.nagi` and its existing Marketplace page. The repository owner handles Marketplace submission; GitHub publication and Marketplace approval/publication are separate.

The `jetbrains-v0.1.1` release contains `nagi-jetbrains-IC-0.1.1.zip` for IDEA and `nagi-jetbrains-PC-0.1.1.zip` for PyCharm, each with a matching `.sha256` file.

To install a GitHub Release ZIP, select it in **Settings → Plugins → ⚙ → Install Plugin from Disk**, then restart the IDE. A PR build is available as the `release-jetbrains` artifact only after all IDEA/PyCharm stable/EAP checks pass in **Actions → Nagi checks**. For a local build, use the ZIP in `build/distributions/`.

Set the compiler executable in **Settings → Languages & Frameworks → Nagi**. An empty value uses `nagic` from `PATH`. Relative paths resolve from the IDE project root.

Open a Nagi file and choose **Nagi: Check** or **Nagi: Run** in its context menu or the Tools menu. These actions save open files before execution. The compiler does not run in untrusted projects.

In 0.1.2, the button beside `main` invokes the same **Nagi: Run** action. It does not use the Python or other run configuration selected in the top toolbar. When a nearby `nagi.toml` exists, it runs that project's entry point. The next-version candidate also exposes this Nagi run through a standard IDE Run Configuration with normal Run/Stop controls.

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

## Minimum IDE for 0.1.2

The common 0.1.2 plugin targets IntelliJ IDEA 2025.1.1 build 251.25410.109 or later and PyCharm 2025.1.1 build 251.25410.122 or later. The initial 2025.1 builds do not resolve the public trust API used to reject commands in untrusted projects. Version 0.1.2 drops 2024.3 and those initial builds. Users on excluded IDE builds can keep the published 0.1.1; upgrade to the supported build before updating the plugin. The ID `com.disnana.nagi` and existing Marketplace page remain unchanged. This support range applies to the GitHub 0.1.2 release.

Definition navigation to virtual standard-library sources and record-field declarations is not supported yet.
