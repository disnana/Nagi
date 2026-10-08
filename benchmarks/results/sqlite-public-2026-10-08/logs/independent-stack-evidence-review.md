# Independent PR #99 Stack/Evidence Review

Read-only review of immutable head `f4166ee4a243d19556902767a3f14c6506a8798f`, PR base `bc6a76bf2422aadaca799b739ab75e5f1e6bc450`, ultimate main `676576724829e45b077b58628bfe2417e6cf3673`. The worktree was clean. Scope was limited to the five rebased CI/navigation files and the result, handoff, artifact-provenance, and evidence links; the SQLite compiler/runtime implementation was not re-reviewed.

## Findings

No actionable corrections found.

## Checks

- Both onboarding and SQLite paths remain present. Japanese and English Docs indexes still lead through setup, language guide, and first CLI app; the contributing walkthrough remains linked. The application indexes distinguish the existing SQLite API from the unreleased Pool/Tx API. Website navigation includes `first-app`, `contributing`, and `sqlite-pool` in both languages. The Task result-handle docs still describe it as released in 0.1.11, and website navigation does not label it unreleased.
- CI keeps the onboarding verifier alongside SQLite checks. The Pages workflow runs both `verify_onboarding_examples.py --docs-only` and `verify_sqlite_example.py --docs-only`. The stack logs report 10 onboarding native cases, Japanese/English SQLite tutorial code matching the executable example, a 98-page website build with local links/anchors/assets verified, and 59 CI-policy tests passing.
- Result/handoff claims for the integrated tree match their corresponding narrow logs. PR #98's four-OS onboarding evidence is explicitly separated from PR #99 evidence. The earlier 94-page website and SQLite SQL-preflight logs are identified with tested source `e3e0ea3`; the later 98-page site and onboarding checks are identified as stack integration `fbfebd3`. The preflight log shows three literal SQL checks, zero unsupported sites, and three runtime bind-unchecked notices.
- `stack-markdown-links.json` reports 2,550 relative file links and `missing: []`; the website log separately covers anchors and assets. External URLs are not fetched by these checks.
- Verified all 70 artifact evidence SHA-256 entries and all 68 tested-source file SHA-256 entries in `provenance.json`; every digest matched. The recorded production-scope diff from tested source `e3e0ea3` to stack source `fbfebd3` is empty for compiler, runtime, `Cargo.lock`, release/distribution gates, and the SQLite cost harness.
- The result and handoff records defer final PR #99 head/CI status to the PR body and Checks; they do not turn the artifact-capture pending status into a final CI pass. No Cargo, native, website, or CI run was executed for this review.

## Prerequisite-path follow-up (2026-10-08)

A narrow beginner-path recheck found a **P2 gap, now resolved in the current working Docs diff**: the prerequisites had referred to “this repository worktree” without telling a reader who only had the published 0.1.11 compiler how to obtain a checkout containing the unreleased API and runnable sample. Japanese [docs/sqlite-pool.md:20](/workspace/Nagi-sqlite-public/docs/sqlite-pool.md:20) and English [docs/en/sqlite-pool.md:20](/workspace/Nagi-sqlite-public/docs/en/sqlite-pool.md:20) now define the required source checkout and name `examples/sqlite_pool.nagi` plus `std.db.sqlite`; the following paragraph links to each language's source-build instructions, directs readers to a development checkout if `main` lacks the sample/API, and states that the published 0.1.11 binary alone is insufficient. The wording is not pinned to PR #99 and will remain valid after merge. The referenced anchors correspond to the existing source-build headings. No repository files were changed by this review; parent is rerunning the lightweight Docs/site checks.
