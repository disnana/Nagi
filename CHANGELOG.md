# Changelog

## Unreleased

- Reject non-printable values in `print` and `write` during type checking, including `unit`, function values, byte/list views, and timestamps. Keep numeric, boolean, string, borrowed string, and UUID output supported.

- Use the Marketplace publisher `Disnana` and the extension name as the VSIX manifest identity. The extension ID is `Disnana.nagi-language`. Verify the identity and build a VSIX artifact for extension changes without publishing unchanged versions.
- Keep the Docs sidebar's scroll position when changing pages. Separate introductory guides from language references, list built-in argument types, and clarify explanations and runnable examples in both languages.
- Distinguish missing Cargo from other launch failures. Preserve build diagnostics without treating every Cargo failure as a rejected Nagi program.
- Separate installer success, next steps, and app build prerequisites. Use terminal colors where available and keep plain-text labels when colors are disabled.
- Re-running the installer updates to the latest published Nagi release. Keep a fixed command on PATH, restore the previous command if activation fails, and remove unchanged older distributions only after a successful update. Explicit version selection remains available.
- Check entry-point and HTTP handler signatures before invoking Rust, with diagnostics pointing to the original Nagi or Low file. Reject duplicate HTTP routes and multiple request-body parameters.
- Read an `id` query parameter when the route has no path capture, instead of returning an HTTP 500 response.
- Build source files whose names contain punctuation, emoji, or decomposed accents.
- Use the installed `nagic` command throughout the introductory language and HTTP guides.
- Cache native dependencies during distribution verification and check HTTP route parameters on all four target platforms.

Published versions and downloads are listed in [GitHub Releases](https://github.com/disnana/Nagi/releases).
