# Proposed Nagi LTS candidate policy

[日本語](lts-candidate.md) · [Candidate validation](lts-candidate-validation.md)

This is an **unadopted proposal**, dated 2026-10-25. It does not designate 0.2.2 as LTS or promise a support period. The published compiler is 0.2.1. Version changes, tags and formal publication are outside this task.

## Sustainable scope

Normally maintain one series. Propose **six months** from an explicitly approved LTS designation, with no automatic extension. Decide extensions separately using operational history, dependency maintenance and available maintainers. Only during migration, and after approval of additional maintenance capacity, a successor may overlap for at most three months within the old series' remaining term, with at most two maintained series. Without that capacity, adjust the successor's start to avoid overlap. Its publication must not shorten the old end date.

Before designation, approve an owner and backup, resources for dependency monitoring and four-platform validation, exact dates and distribution targets. Without these resources, use an ordinary stable release. Do not promise multi-year coverage or an always-on SLA in advance.

## Compatibility boundary

- Preserve the designated public High/Low syntax, type/ownership/capability semantics, standard APIs, documented CLI/project configuration and observable valid-input behavior. This does not restore contracts already migrated before designation.
- Preserve default release builds, default `--runtime-profile full`, complete checks, Cargo native freshness, lock consistency and previous successful executables. Changes making auto, body CAS or units the default belong to development.
- Preserve the original `nagi-runtime` Rust API, including no-default-features consumers, and tested compiler/runtime combinations. Arbitrary handwritten Rust dependencies or mixed runtime versions are outside this guarantee.
- Cover the tested Linux x86_64, Windows x86_64 and macOS ARM64/Intel distributions. Record the minimum Rust version, OS and TLS/native requirements at designation and retest them for patches. Do not promise a stable Rust ABI or compatibility of arbitrary third-party crates.
- Internal IR, generated Rust layout, caches and private implementation are not stable APIs. Regenerate mismatched caches and use full when optimization safety cannot be established. Persistent-data changes need migration, backup and failure handling.

JetBrains and VS Code extensions have independent versions. Publish an IDE/compiler compatibility matrix and feature-detect compiler flags. Compiler LTS does not imply compatibility with every future IDE version.

## Fixes and backports

Limit LTS changes to confirmed reachable security problems, miscompilation, data corruption, crashes/hangs, public-contract regressions and necessary dependency/distribution compatibility fixes. New syntax, broad APIs, cache/query redesign and experimental performance features stay on development. Consider small performance fixes only with comparable measurements and regression evidence.

Normally reproduce and fix on main, then backport the smallest change to `lts/<series>`. For private security fixes, follow the existing [private reporting policy](../../SECURITY.en.md); do not expose undisclosed details in public issues or PRs. Do not introduce response or repair deadlines. Assess advisories against actual locked versions, features, call paths and deployment conditions. Unknown reachability remains unknown; do not claim zero vulnerabilities.

Record affected versions, originating commit, excluded series and reasons, reproduction, relevant High/saved-Low/native regressions, independent review and exact-head CI. Dependency updates include Cargo.lock, default/optimized paths, minimum Rust and four-platform L3 distribution validation. Do not remove tests, add skips or weaken judgments to pass.

When safety and compatibility conflict, seek a separate decision with impact, mitigations and migration. Do not silently change contracts or hide an unfixable issue to maintain the LTS label. A compiler update does not update application-specific Cargo.lock files or existing executables; explain when dependency resolution and rebuilding are necessary.

## Development and end of support

After designation, use `lts/<series>` for fixes and main for development, with version-specific distribution pages, Docs and AI guidance. This proposal creates no maintenance branch or publication settings. Explain why features are not backported and which release provides them.

Agree the end date before designation. Propose a reminder at least 30 days before it, listing the final patch, known issues and migration path. No approved extension means no extension. Private reporting continues under the existing policy, without a promise to patch an ended series. Escalate loss of maintenance capacity promptly rather than independently shortening an existing commitment.

## Designation decision

L3 success is necessary distribution evidence, not proof of maintenance capacity or long-term stability. Use the [validation record](lts-candidate-validation.md), including its unverified items, to separately approve:

1. Final four-platform/MSRV, browser/TLS, distribution, install/upgrade/uninstall and existing security/compatibility gates, plus representative full/auto/saved-Low/native equivalence.
2. Separate build, execution, test and CI observations, including regressions, fallback and unmeasured cases.
3. Owners and investigation or explicitly accepted residual risk for historical Windows/SF06 failures. A passing rerun does not establish a root-cause fix.
4. Correct public Docs/SECURITY/editor version statements, maintainers, support scope, dates and end conditions.

Until these conditions and approval are complete, retain the candidate label. Evaluate an ordinary 0.2.2 release separately from LTS designation.
