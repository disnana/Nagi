# Nagi for JetBrains

[日本語](README.md)

Nagi support for IntelliJ IDEA and PyCharm. Install the Nagi compiler separately.

## Features

- Syntax highlighting for High (`.nagi`) and Low (`.low`).
- Line comments, matching and paired brackets, and block folding.
- Indentation on Enter: High definitions such as `def main():` and `case`, Low braces, and multiline parenthesized expressions.
- **Nagi: Check** and **Nagi: Run**. The nearest `nagi.toml` selects a project; otherwise the selected source file is used.
- Click compiler source locations in the Run console to open the file.

Whole-file formatting, semantic completion, go to definition, and automatic checks while typing are planned. Type checking uses the Nagi compiler.

## Installation

On GitHub, open a successful **Actions → Nagi checks** run (PR) or **Nagi JetBrains plugin** run (main) and download the `nagi-jetbrains-IC` (IDEA) or `nagi-jetbrains-PC` (PyCharm) artifact. Extract that download to find the plugin ZIP.

Select `nagi-jetbrains-*.zip` in **Settings → Plugins → ⚙ → Install Plugin from Disk**, then restart the IDE. For a local build, use the ZIP in `build/distributions/`. The plugin has not been published to the Marketplace.

Set the compiler executable in **Settings → Languages & Frameworks → Nagi**. An empty value uses `nagic` from `PATH`. Relative paths resolve from the IDE project root.

Open a Nagi file and choose **Nagi: Check** or **Nagi: Run** in its context menu or the Tools menu. These actions save open files before execution. The compiler does not run in untrusted projects.

Output appears in the Run window. Check has a 30-second timeout, configurable from 1 to 300 seconds. Run has no timeout, so servers can keep running; use Stop in the Run window to end the process. Generated files are stored under `nagi` in the IDE system directory.

Building and running Nagi applications also requires Rust/Cargo and the build tools for your OS.

## Build and verification

Use JDK 21.

```sh
cd editors/jetbrains-nagi
./gradlew test buildPlugin
```

On Windows, use `gradlew.bat`. The wrapper pins Gradle 8.13, IntelliJ Platform Gradle Plugin 2.3.0, and IntelliJ IDEA Community 2024.3.7 as the default SDK. The first build downloads the SDK and dependencies.

The same code can be checked against the PyCharm SDK:

```sh
./gradlew -PplatformType=PC -PplatformVersion=2024.3.6 test buildPlugin
```

Use `-PlocalPlatformPath=/path/to/ide` to build against a local IDE. `runIde` starts an isolated development IDE.

Tests cover scanning, folding, indentation, CLI argument boundaries, diagnostic locations, and real IntelliJ Platform editor fixtures for file types, Enter, comments, paired brackets, and save failures. A real process also tests cancellation during startup. Set `NAGI_TEST_COMPILER` to an installed `nagic` executable to check High, Low, and project commands with the compiler; only this extra smoke is skipped when unset.

Editor fixtures and distribution ZIP generation have been checked against the IntelliJ IDEA Community 2024.3.7 SDK. Complete IDE interaction, Plugin Verifier compatibility checks, and PyCharm verification are separate checks. CI uses complete IDEA and PyCharm SDKs for tests, ZIP generation, and compatibility verification.

Run `./gradlew test buildPlugin verifyPlugin` to perform the same checks as CI. `verifyPlugin` checks compatibility with the selected IDE's APIs.

Official references: [Plugin SDK](https://plugins.jetbrains.com/docs/intellij/developing-plugins.html), [Gradle Plugin](https://plugins.jetbrains.com/docs/intellij/tools-intellij-platform-gradle-plugin.html). Licensed under [MIT](LICENSE).
