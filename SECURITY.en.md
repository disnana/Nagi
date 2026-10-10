# Security policy

[日本語](SECURITY.md)

## Report a vulnerability

Use [GitHub's private reporting form](https://github.com/disnana/Nagi/security/advisories/new). You need to sign in to GitHub. Keep details and proof-of-concept code for unfixed vulnerabilities out of public issues and PRs.

Include the affected version or commit, OS, minimal reproduction steps, impact, and conditions required to trigger the problem. Reproduce it in your own environment and remove secrets and third-party data.

We will investigate reports and coordinate fixes and disclosure with the reporter. We do not guarantee response or fix deadlines. Use [Issues](https://github.com/disnana/Nagi/issues) for ordinary bugs and questions.

## Scope

This policy covers the Nagi compiler, runtime, VS Code and JetBrains extensions, distributions, and release or deployment processes. We first check the impact on `main` and the latest published release. Reports affecting older versions are welcome; backports are considered individually.

Report information disclosure, unintended execution, data corruption, or denial of service caused by external input such as HTTP or JSON, and execution that bypasses editor trust settings. For dependency vulnerabilities, include the path through which Nagi is affected.

## Assumptions and current limits

The latest published release is Nagi 0.1.11. GitHub main `e609aba158921226a632f16d41eb8b0f4ad5aebd` is development source; formal 0.2.0 is unreleased. The published 0.1.11 distribution and its contracts are unchanged. `nagic build/run` and Rust integration do not isolate the code they run. Test untrusted source, `nagi.toml`, and Rust dependencies in an isolated environment without secrets or important files.

Published 0.1.11 does not include development main's request-bound `std.auth` contract, required explicit `std.http.Policy` on each standard route, or the new `std.db.sqlite.Query`/`Parameters` contract. Development main binds `AuthScope`/`Grant` to a request and requires an explicit policy for each standard route (`public` is an explicit anonymous policy). Its standard SQLite path uses canonical literal Query and Parameters for the SQL-structure/value-binding boundary. These constrain the applicable standard API paths; they do not prove application policy logic, tenant authorization, arbitrary Rust behavior, or whole-application safety. SF02/SF03/SF04/SF06, SF07's cross-cutting budget acceptance, and SF08 remain incomplete; formal 0.2.0 has not been released.

VS Code checks workspace trust and JetBrains checks project trust before starting the compiler. The resulting program runs with the same permissions as an ordinary application.

Development main's standard `std.http.server` binds to `127.0.0.1` and has deadlines for header/keep-alive waiting, body reception, and handlers, plus body limits. It also exposes connection/request capacities and send/shutdown deadlines. These settings and limits differ from published 0.1.11's legacy `serve(Db, port)`. See [standard HTTP limits](docs/en/http-server.md) and the [legacy API load tests](docs/en/http-capacity.md).

These limits do not provide application authentication, authorization, or rollback. Accepted database writes and other work may finish after cancellation. See the [RFC](docs/internal/security-foundation/rfc.en.md) and [implementation plan](docs/internal/security-foundation/implementation-plan.en.md) for development-main scope and open work, and the [CHANGELOG](CHANGELOG.md) for differences from published releases.

For external deployment, use a front proxy or equivalent to manage TLS, connection counts, header and idle deadlines, and traffic rates. Implement application authentication and authorization. DDoS that saturates a network link also requires protection from the hosting provider or CDN.
