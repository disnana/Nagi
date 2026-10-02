# Changelog

## Unreleased

- Use the Marketplace publisher `Disnana` and the extension name as the VSIX manifest identity. The extension ID is `Disnana.nagi-language`. Verify the identity and build a VSIX artifact for extension changes without publishing unchanged versions.

- Separate installer success, next steps, and app build prerequisites. Use terminal colors where available and keep plain-text labels when colors are disabled.
- Re-running the installer updates to the latest published Nagi release. Keep a fixed command on PATH, restore the previous command if activation fails, and remove unchanged older distributions only after a successful update. Explicit version selection remains available.
- Check entry-point and HTTP handler signatures before invoking Rust, with diagnostics pointing to the original Nagi or Low file. Reject duplicate HTTP routes and multiple request-body parameters.
- Read an `id` query parameter when the route has no path capture, instead of returning an HTTP 500 response.
- Build source files whose names contain punctuation, emoji, or decomposed accents.
- Use the installed `nagic` command throughout the introductory language and HTTP guides.
- Cache native dependencies during distribution verification and check HTTP route parameters on all four target platforms.

Published versions and downloads are listed in [GitHub Releases](https://github.com/disnana/Nagi/releases).
