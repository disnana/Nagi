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

Use JDK 21 for stable SDK checks and the common ZIP candidate build. EAP checks use the JDK required by the resolved SDK. The current 2026.3 EAP requires JDK 25, so EAP verification needs JDK 25. The plugin Java API and class-file target remain fixed at `options.release=21` even when verification runs on JDK 25.

```sh
cd editors/jetbrains-nagi
./gradlew test buildPlugin
```

On Windows, use `gradlew.bat`. The wrapper pins Gradle 9.4.0, IntelliJ Platform Gradle Plugin 2.15.0, and IntelliJ IDEA Community 2025.1.1 as the default SDK. The first build downloads the SDK and dependencies.

The same code can be checked against the PyCharm SDK:

```sh
./gradlew -PplatformType=PC -PplatformVersion=2025.1.1 test buildPlugin
```

Use `-PlocalPlatformPath=/path/to/ide` to build against a local IDE. `runIde` starts an isolated development IDE.

Tests cover scanning, folding, indentation, CLI argument boundaries, diagnostic locations, and real IntelliJ Platform editor fixtures for file types, Enter, comments, paired brackets, and save failures. A real process also tests cancellation during startup. Set `NAGI_TEST_COMPILER` to a `nagic` executable to check High, Low, and project commands with the compiler. JetBrains CI builds the compiler from the same commit and runs this integration test for both IDEA and PyCharm.

The minimum target for the next version is 2025.1.1: IDEA build `251.25410.109` and PyCharm build `251.25410.122` were verified. The initial 2025.1 builds (`251.23774`) do not expose the public API used by the trusted-project check; Plugin Verifier reports an unresolved method, so those builds are excluded. CI first builds one common ZIP candidate against stable IDEA on JDK 21. It runs IDE tests from the same source on four SDKs—stable 2025.1.1 and EAP for both IDEA and PyCharm—and passes the same candidate ZIP to each Plugin Verifier. Stable checks use JDK 21; EAP checks use the JDK required by each resolved SDK (currently JDK 25). The minimum IDEA/PyCharm 2025.1.1 targets are also checked by Plugin Verifier. Only after all four IDE tests and verifiers pass does CI promote the original candidate ZIP to the distribution artifact. The two ZIPs attached to the published 0.1.1 release are historical product-specific assets, separate from this future common ZIP. Marketplace UI installation and full IDE interaction are not covered by these checks.

Run `./gradlew test buildPlugin verifyPlugin` with JDK 21 for stable compatibility checks. When an EAP SDK requires JDK 25, run its test with JDK 25. `options.release=21` keeps the plugin's compile target and Java API surface at 21 on either JDK. The verifier mutes only the `TemplateWordInPluginName` lint for the required display name; API compatibility, deprecated, and experimental findings remain active. Add `-PminimumPlatformVersion=2025.1.1` for IDEA/PyCharm to check the minimum target too. Use `-PplatformVersion=LATEST-EAP-SNAPSHOT` to check the latest EAP. With `-PlocalPlatformPath`, only that local SDK is verified.

Official references: [Plugin SDK](https://plugins.jetbrains.com/docs/intellij/developing-plugins.html), [Gradle Plugin](https://plugins.jetbrains.com/docs/intellij/tools-intellij-platform-gradle-plugin.html). Licensed under [MIT](LICENSE).

## Minimum IDE for the next version

The common plugin under development targets IntelliJ IDEA 2025.1.1 build 251.25410.109 or later and PyCharm 2025.1.1 build 251.25410.122 or later. The initial 2025.1 builds do not resolve the public trust API used to reject commands in untrusted projects. The next version drops 2024.3 and those initial builds. Users on excluded IDE builds can keep the published 0.1.1; upgrade to the supported build before updating the plugin. The ID `com.disnana.nagi` and existing Marketplace page remain unchanged. This support change is unreleased.
