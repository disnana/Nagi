# Nagi for JetBrains

[日本語](README.md)

Nagi support for IntelliJ IDEA and PyCharm. [Install the compiler separately](https://nagi.disnana.com/en/docs/getting-started/).

The plugin version is managed in `build.gradle.kts`. The published version is 0.1.1. Install it directly from [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/34891-nagi) in a compatible IDE, or download an IDE-specific ZIP from [GitHub Releases](https://github.com/disnana/Nagi/releases).

## Features

- Syntax highlighting for High (`.nagi`) and Low (`.low`).
- Line comments, matching and paired brackets, and block folding.
- Indentation on Enter: High definitions such as `def main():` and `case`, Low braces, and multiline parenthesized expressions.
- **Nagi: Check** and **Nagi: Run**. The nearest `nagi.toml` selects a project; otherwise the selected source file is used.
- A run button beside top-level `def main()`, `async def main()`, and Low `fn main()` declarations.
- Click compiler source locations in the Run console to open the file.

Whole-file formatting, semantic completion, go to definition, and automatic checks are not supported. Type checking invokes the Nagi compiler manually.

## Installation

The `jetbrains-v0.1.1` release contains `nagi-jetbrains-IC-0.1.1.zip` for IDEA and `nagi-jetbrains-PC-0.1.1.zip` for PyCharm, each with a matching `.sha256` file.

To install a GitHub Release ZIP, select it in **Settings → Plugins → ⚙ → Install Plugin from Disk**, then restart the IDE. A PR build is available as the `release-jetbrains` artifact only after all IDEA/PyCharm stable/EAP checks pass in **Actions → Nagi checks**. For a local build, use the ZIP in `build/distributions/`.

Set the compiler executable in **Settings → Languages & Frameworks → Nagi**. An empty value uses `nagic` from `PATH`. Relative paths resolve from the IDE project root.

Open a Nagi file and choose **Nagi: Check** or **Nagi: Run** in its context menu or the Tools menu. These actions save open files before execution. The compiler does not run in untrusted projects.

The button beside `main` invokes the same **Nagi: Run** action. It does not use the Python or other run configuration selected in the top toolbar. When a nearby `nagi.toml` exists, it runs that project's entry point.

Output appears in the Run window. Check has a 30-second timeout, configurable from 1 to 300 seconds. Run has no timeout, so servers can keep running; use Stop in the Run window to end the process. Generated files are stored under `nagi` in the IDE system directory.

Building and running Nagi applications also requires Rust/Cargo and the build tools for your OS.

## Build and verification

Use JDK 21.

```sh
cd editors/jetbrains-nagi
./gradlew test buildPlugin
```

On Windows, use `gradlew.bat`. The wrapper pins Gradle 9.4.0, IntelliJ Platform Gradle Plugin 2.14.0, and IntelliJ IDEA Community 2025.1.1 as the default SDK. The first build downloads the SDK and dependencies.

The same code can be checked against the PyCharm SDK:

```sh
./gradlew -PplatformType=PC -PplatformVersion=2025.1.1 test buildPlugin
```

Use `-PlocalPlatformPath=/path/to/ide` to build against a local IDE. `runIde` starts an isolated development IDE.

Tests cover scanning, folding, indentation, CLI argument boundaries, diagnostic locations, and real IntelliJ Platform editor fixtures for file types, Enter, comments, paired brackets, and save failures. A real process also tests cancellation during startup. Set `NAGI_TEST_COMPILER` to a `nagic` executable to check High, Low, and project commands with the compiler. JetBrains CI builds the compiler from the same commit and runs this integration test for both IDEA and PyCharm.

The primary 2025.1.1 target is an existing supported baseline. CI first builds one common ZIP candidate against stable IDEA, then runs the IDE tests and Plugin Verifier against that same candidate on four paths: stable 2025.1.1 and EAP for both IDEA and PyCharm. The minimum IDEA 2024.3.7 and PyCharm 2024.3.6 targets (build 243) are checked by Plugin Verifier only; CI does not run IDE tests on those minimum versions. Only after all four paths pass does CI promote the original candidate ZIP to the distribution artifact. The two ZIPs attached to the published 0.1.1 release are historical product-specific assets, separate from this future common ZIP. These checks are separate from interacting with the complete IDE.

Run `./gradlew test buildPlugin verifyPlugin` for compatibility checks. The verifier mutes only the `TemplateWordInPluginName` lint for the required display name; API compatibility, deprecated, and experimental findings remain active. Add `-PminimumPlatformVersion=2024.3.7` for IDEA, or `-PplatformType=PC -PminimumPlatformVersion=2024.3.6` for PyCharm, to check the minimum target too. Use `-PplatformVersion=LATEST-EAP-SNAPSHOT` to check the latest EAP. With `-PlocalPlatformPath`, only that local SDK is verified.

Official references: [Plugin SDK](https://plugins.jetbrains.com/docs/intellij/developing-plugins.html), [Gradle Plugin](https://plugins.jetbrains.com/docs/intellij/tools-intellij-platform-gradle-plugin.html). Licensed under [MIT](LICENSE).
