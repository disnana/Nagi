# Security policy

[日本語](SECURITY.md)

## Report a vulnerability

Use [GitHub's private reporting form](https://github.com/disnana/Nagi/security/advisories/new). You need to sign in to GitHub. Keep details and proof-of-concept code for unfixed vulnerabilities out of public issues and PRs.

Include the affected version or commit, OS, minimal reproduction steps, impact, and conditions required to trigger the problem. Reproduce it in your own environment and remove secrets and third-party data.

We will investigate reports and coordinate fixes and disclosure with the reporter. We do not guarantee response or fix deadlines. Use [Issues](https://github.com/disnana/Nagi/issues) for ordinary bugs and questions.

## Scope

This policy covers the Nagi compiler, runtime, VS Code extension, distributions, and release or deployment processes. We first check the impact on `main` and the latest published release. Reports affecting older versions are welcome; backports are considered individually.

Report information disclosure, unintended execution, data corruption, or denial of service caused by external input such as HTTP or JSON, and execution that bypasses VS Code workspace trust. For dependency vulnerabilities, include the path through which Nagi is affected.

## Assumptions and current limits

Nagi is in development in the 0.1 series. `nagic build/run` and Rust integration do not sandbox arbitrary programs. Handle untrusted source, `nagi.toml`, and Rust dependencies in an isolated environment without secrets or important files. The VS Code extension invokes the compiler only in trusted workspaces.

The HTTP server binds to `127.0.0.1` by default. Deadlines for silent connections, incomplete headers, and unused keep-alive connections are not implemented. The existing handler timeout does not prevent these connections from occupying resources. See the [measurements and limitations](docs/en/http-capacity.md).

For external deployment, use a front proxy or equivalent to manage TLS, connection counts, header and idle deadlines, and traffic rates. Implement application authentication and authorization. DDoS that saturates a network link also requires protection from the hosting provider or CDN.
