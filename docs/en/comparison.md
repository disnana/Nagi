# Nagi's strengths and language comparisons

Nagi is for developers who want to **write typed backend logic with readable, indentation-based syntax and use Rust's execution foundation and libraries**. Published 0.2.3 includes HTTP, JSON, SQLite, Results, ownership/borrowing, async, and actors, plus explicit opt-in runtime selection and an initial lock seed. The planned 0.3.0 Go target is not a published backend and does not yet provide Go HTTP or Actor support.

## What Nagi offers

| Strength | Practical value | Try it or inspect the evidence |
|---|---|---|
| Indented syntax with explicit types and failures | Keep inputs, outputs, and failures in small functions that people and coding agents can follow | [First CLI app](first-app.md), [Results](error-handling.md) |
| Checks before Rust compilation | Use `nagic check` after editing to find Nagi syntax, type, and ownership errors first | [Development quick reference](developer-workflow.md) |
| Native execution and Rust assets | Compile generated Rust into executables; use crates through small typed adapters | [Rust integration](modules-and-rust.md), [Axum application](../../test-nagi-code/application-examples/axum-service/README.en.md) |
| Explicit boundaries in standard APIs | Express HTTP budgets, authentication/authorization, and SQL binding through APIs and types | [HTTP](http-server.md), [authentication](security.md), [SQLite](sqlite-pool.md) |
| A development build path | Dev profiles, shared Cargo dependencies, change observations, and opt-in generated-body/function-group reuse | [Development builds](development-builds.md), [application CI](user-ci.md) |
| Instructions for coding agents | Use a skill and task-specific references covering supported syntax, diagnosis, and verification | [Agent documentation](../../ai/README.md) |

For example, this typed calculation prints `42`:

```nagi
def double(value: i64) -> i64:
    return value * 2

def main():
    print(double(21))
```

This is not Python-compatible code: Nagi has its own types, move/view, and Result rules. Choose HTTP, database, and async examples from the [working samples](library-examples.md).

## Compared with Python/FastAPI, Rust/Axum, and Go

This table compares development approaches, including language/framework combinations. It is not a speed ranking.

| Decision | Nagi 0.2.3 | Python + FastAPI | Rust + Axum | Go + `net/http` |
|---|---|---|---|---|
| Writing code | Indented syntax, typed functions, Results, move/view | Indented syntax, type annotations, Pydantic input models | Rust types, ownership, traits, Results, async | Explicit types, structs/interfaces, errors, goroutines |
| Feedback after editing | Complete Nagi `check`, then Cargo builds when needed | Python execution, type-checking tools, tests; normally no native application build | `cargo check`, `cargo test`, execution | `go test`, `go run`, `go build` |
| Distribution and execution | Native executable compiled from generated Rust; check target OS/native dependencies | Python environment, application dependencies, ASGI server | Native executable; check target OS/native dependencies | Native executable; check native dependencies when using cgo and similar integrations |
| Memory | Nagi ownership checks and Rust's final checks; shared APIs use Arc and related mechanisms | Managed objects; CPython uses reference counting and cyclic GC | Ownership/borrowing, Arc where needed | GC and goroutine runtime |
| Libraries | Nagi standard APIs and explicit Rust adapters; no direct Python package use | PyPI and Python libraries | crates.io and Rust libraries | Go modules and standard library |
| Where to evaluate it | Small backends/CLIs and experiments combining typed business logic with Rust assets | APIs/data processing centered on Python assets | Services needing Rust's full expressiveness, control, and ecosystem | Services using Go assets, standard tools, and concurrency |

Official references: [FastAPI types and validation](https://fastapi.tiangolo.com/python-types/), [Python memory management](https://docs.python.org/3/c-api/memory.html), [Rust ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html), [Cargo check](https://doc.rust-lang.org/cargo/commands/cargo-check.html), [Axum](https://docs.rs/axum/latest/axum/), [Go commands](https://pkg.go.dev/cmd/go), [Go HTTP](https://pkg.go.dev/net/http), and [Go GC](https://go.dev/doc/gc-guide). The table does not represent every implementation or configuration of each language.

## Choose a first use case

Start with clearly defined inputs, outputs, and APIs: [JSON validation and aggregation](../../test-nagi-code/application-examples/stock-report/README.en.md), [SQLite-backed settings](../../test-nagi-code/application-examples/device-settings/README.en.md), or [business functions called from Rust/Axum](../../test-nagi-code/application-examples/axum-service/README.en.md). Test accepted and rejected inputs, and check which needs are covered by standard APIs versus adapters.

Methods, inheritance, general traits, and a standard PostgreSQL API are currently unsupported. Python libraries and arbitrary Rust types cannot be imported directly. This page does not claim established large-scale deployments or maturity equivalent to the other languages. Inspect [syntax](syntax.md), [implemented scope](introduction.md), and [future plans](roadmap.md) separately.

## Read performance with its conditions

Generating native code through Rust alone does not establish parity with handwritten Rust or a speed multiplier over Python/Go. HTTP, JSON, database work, allocations, cloning, and async conditions need matched measurements.

Existing [HTTP measurements](http-stdlib-performance.md), [build comparisons](../../benchmarks/results/smart-build-2026-10-20/README.md), and [function-unit measurements](../../benchmarks/results/function-unit-reuse-2026-10-21/README.md) include conditions and results. Function partitioning improved unchanged builds in the measured case while some edited builds became slower. Initial builds compile Cargo dependencies, unlike simply starting a Python application.

This page does not present a comprehensive matched Nagi/FastAPI/Axum/Go benchmark. Follow the [measurement guide](performance.md) to compare build time, response time, memory, and required code/tests for your own application.

The [concrete task API comparison](task-api-comparison.md) aligns a SQLite-backed JSON API across Nagi, FastAPI, and Quart, including validation, servers, dependencies, and distribution. It is not a load benchmark.
