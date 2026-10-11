# Codex agent routing and verification

2026-10-11 JST. [AGENTS.md](../../AGENTS.md) sets the task policy; [.codex/config.toml](../../.codex/config.toml) caps child threads. [Light Worker](../../.codex/agents/light_worker.toml), [Fast Worker](../../.codex/agents/fast_worker.toml), [Balanced Engineer](../../.codex/agents/balanced_engineer.toml), [Deep Engineer](../../.codex/agents/deep_engineer.toml), [Independent Reviewer](../../.codex/agents/independent_reviewer.toml), [Architect](../../.codex/agents/architect.toml), [Critical Reviewer](../../.codex/agents/critical_reviewer.toml), and [Astra Final Verifier](../../.codex/agents/astra_final_verifier.toml) define the role settings.

## Route by task

Choose the lowest-cost route likely to meet required correctness and audit evidence. Include model use, test/CI execution and waiting, failed attempts, rework, context transfer, and review effort in end-to-end time/cost. Rates alone do not show whole-task cost; subjective speed is not a measurement.

| Work | Default | Move up when |
|---|---|---|
| Few-line change | Parent does it directly | An independent deliverable appears |
| Bounded docs, rename, read-only question | Luna Medium / Light Worker | Scope or contract decision expands |
| Clear feature, fixture, ordinary test, obvious CI cause | Luna Max / Fast Worker | Same-cause targeted fix fails twice or code/contract boundary is unclear |
| Clear multi-file implementation or investigation where Luna rework may dominate | GPT-6 Sol Max / Balanced Engineer | Unknown cause or important semantics need Sol 6.1 High; final important-code audit uses a separate Sol 6.1 agent |
| Important or difficult code; multi-module investigation | Sol 6.1 High / Deep Engineer | Unresolved semantics, compatibility, race, or lifecycle design needs Architect |
| Auth, authorization, secrets, integrity, concurrency, destructive change | Sol 6.1 High confirms approved design before implementation; xHigh only for unresolved semantics/race/alternatives; Sol 6.1 High implements | Independent Sol 6.1 High audit after checks; fix, rerun affected regressions, and re-review |
| Independent final audit on important code | Sol 6.1 High / Independent Reviewer, after checks | Fix findings, rerun affected regressions, and return final diff/results for re-review |
| Exceptional unresolved high-cost issue | Sol 6.1 Max; Astra Max only if Sol Max is insufficient | Do not escalate without a concrete question |

For high-risk work, Sol 6.1 confirms the approved design and failure/regression cases first; xHigh Architect is reserved for unresolved design. The independent reviewer then compares final diff, specification, results, and relevant regressions. Fixes require targeted reruns and a final re-review. Source-only review is not a substitute for execution.

For compiler changes, distinguish High/Low parse, checker outcome, negative reason and primary line, emitted Rust build, and a minimal native test where emission/runtime is affected. Mark blocked stages unverified and keep exact-head CI as a completion condition. When available, record model time, engineering time, test/CI runtime, CI wait, and rework separately; unavailable values stay unknown. Reuse results for the same source/head and rerun only for a relevant change or new evidence.

On 2026-10-11 the user reported a favorable cost/speed balance for GPT-6 Sol Max. This is a reason to consider it for bounded development, not measured latency or total-cost evidence. The supplied Standard credit rates for GPT-6 Sol and GPT-6.1 Sol have equal input/output rates; GPT-6.1 has a lower cached-input rate. Reasoning use and rework can change whole-task cost, so do not claim GPT-6 Sol Max is cheaper from token rates alone. GPT-6 Sol Max is an alternative development route; GPT-6.1 Sol Max remains an exceptional reviewer, not the same role. Reassess using completion time, rework and independent audit findings. Existing extra-charge and Fast permissions are unchanged.

## Observations and joint reassessment

This document is the separate record linked from AGENTS.md; do not create a file per routing decision. Add a short representative observation at a milestone or a consequential failure. Retain useful contrasting cases, not full conversations or secrets. Record task class and exact source/HEAD, model/effort and whether speed controls exist, instructions/context/tools/environment, measured task time, test/tool runtime, CI runtime and queue/wait, revision cycles, independent findings and executed validation stages. Model-only time, token use, credits and API USD stay unknown unless directly observable. Nested timings are not additive: a test duration can be part of the overall task interval.

Consider unclear instructions, incomplete context, scope, stale sources, fixture/test omissions, cache state, resource/network limits, task difficulty and differences in review depth before attributing a result to the model. A local test pass does not establish whole-PR readiness. Prefer a conservative known route while evidence is weak; change one routing or instruction variable at a time when practical during necessary work. Do not rerun the same task on several models just to manufacture a benchmark. Reassess on repeated rework, a material miss, changed availability/pricing or several comparable task observations; do not wait for a quota before escalating a concrete risk.

For a material routing-policy change, give Luna and Sol 6.1 the same evidence and question independently, then exchange their arguments once. Each can disagree on equal terms; model prestige is not a tie-breaker. The parent records adopted/rejected/unresolved proposals and reasons. Agreement between models does not prove correctness or replace tests or independent code audit. Routine dispatch uses the defaults without a meeting. Keep independent development moving within the concurrency cap; only the designated owner edits shared policy files.

### 2026-10-11 observations and decision

| Evidence | Observation | Limits and action |
|---|---|---|
| User report on GPT-6 Sol Max | Favorable cost/speed balance in their experience | Subjective, no controlled latency or billing data. Consider it for clear bounded implementation; do not rank all models from this report |
| PR [#115](https://github.com/disnana/Nagi/pull/115), source `070de14b40218a20ce267b93a2aac928aad4d535`, bounded two-file CI correction | GPT-6 Sol Max task interval reported as 00:33:30–00:38:40 UTC (5m10s); targeted Python run observed 60 passes in 6.502s, included in that interval; 22 pinned dependency blobs checked | Task interval includes tool/test/reasoning work and inherited investigation, not model-only latency. Speed setting unavailable; tokens, credits and USD unknown. Rust/4-OS execution and independent final audit were pending at this observation. One case does not demonstrate superiority |
| PR [#114](https://github.com/disnana/Nagi/pull/114), source `0dd9cf1c651efb3a99c794ccdd516516f7a9374e`, [CI 38098161525](https://github.com/disnana/Nagi/actions/runs/38098161525) | Focused local Session tests passed, but CI found duplicate test-module loading and missing SessionSameSite constants in the independent inventory | Integration coverage was incomplete; broader local checks were also limited by disk capacity. Do not infer a model cause. Add the relevant inventory and harness checks to the next targeted validation; preserve the independent oracle |

Luna Max and Sol 6.1 High independently recommended bounded GPT-6 Sol Max use, preserving strong design/audit gates and distinguishing subjective speed from measurement. After exchanging arguments, both supported a short record in this existing document and opposed per-task meetings or prestige-based ranking. The parent adopts these proposals. Blanket “Sol Max is cheaper/faster”, escalation by file count alone and AI consensus as quality evidence are rejected. Comparative completion cost/latency remains unresolved until sufficiently comparable observations exist. Existing payment, publication and safety constraints are unchanged.

## Ready for review

The parent removes Draft after the PR's accepted scope is complete and the latest HEAD has passed applicable mandatory CI, regressions, independent review and review after fixes. Recheck HEAD/base, review coverage, unresolved findings, mergeability, dependencies and any required GUI/user acceptance immediately before the action. Incomplete stacked dependencies and explicit PR holds keep Draft. This operation is authorized by the user's standing instruction; do not ask again after the conditions are met. Verify GitHub reports `draft=false`, then report the PR, HEAD and evidence. Unknown mergeability prevents the transition. A later HEAD change or regression invalidates readiness; return the PR to Draft when the tool permits and record why. Do not claim the operation happened if the tool is unavailable. Ready status grants no permission to merge, bump versions, tag or publish.

## Concurrency and escalation

`.codex/config.toml` permits two child threads plus the parent (at most three agents); children do not spawn children. Spawn only for independent work with a defined result. Only one agent uses a given model ID plus reasoning effort at a time. Deep Engineer and Independent Reviewer both use Sol 6.1 High and therefore run sequentially.

Luna Medium → Luna Max → GPT-6 Sol Max or GPT-6.1 Sol High → GPT-6.1 Sol xHigh → GPT-6.1 Sol Max → Astra is a possible path, not a required ladder. Start higher when risk warrants it. After two targeted failures from the same cause, or sooner on a specific contract/security uncertainty, stop repeating and pass the exact input, command, diagnostic, source line, and prior results forward. Do not blame the model without evidence. Reassess a route if time/quality/total cost fails to improve.

## Active catalog and service tiers

The current session agent tool catalog exposes `gpt-6-luna` Medium/Max, `gpt-6-sol` Max, `gpt-6.1-sol` High/xHigh/Max, and `gpt-6-astra` Max. Role TOML specifies model and reasoning effort; the current invocation interface has no speed/service-tier option. “Fast Worker” is a role name. Existing explicit approval covers only Luna Max Fast when the environment offers it. Fast for other models, additional purchase/pay-as-you-go/plan/add-on, or another charge beyond current approval requires confirmation. Ordinary task-driven model/effort selection and Sol escalation within current authorization should proceed without another approval stop. Do not add an unsupported service-tier key. Fast’s displayed usage multiplier and `max` reasoning effort do not promise a speed increase.

## Usage-credit evidence and limits

The user-provided compiled report dated 2026-10-11 claims “Business/Enterprise” credits per 1M tokens, ordered input / cached input / output. It is not a verified provider billing/model-price UI; its issuing product and primary URL were not verified. These values are not official API USD prices.

| Report label | Tier | Credits per 1M (input / cached / output) |
|---|---|---:|
| GPT-6 Luna | Standard / Fast | 2.5 / 0.25 / 12.5; 5 / 0.5 / 25 |
| GPT-6.1 Sol | Standard / Fast | 50 / 2.5 / 250; 100 / 5 / 500 |
| GPT-6 Astra | Standard / Fast | 250 / 25 / 1,250; 500 / 50 / 2,500 |
| GPT-6 Sol | Standard / Fast | 50 / 5 / 250; 100 / 10 / 500 |
| GPT-5.6 Sol | Standard; Fast if offered | 100 / 5 / 500; 2× Standard multiplier shown, exact Fast rates unverified |
| GPT-5.6 Terra (report label only) | Standard | 50 / 5 / 300 |
| GPT-5.6 Luna | Standard | 5 / 0.5 / 30 |

The report’s Fast rows claim 2× usage credits for listed models, not 2× measured speed. API USD rates, total session bills, actual latency, and per-task costs were unavailable. Non-role rows above are observations only; verify the active ID/effort before routing to them.

A separate user-provided compiled report dated 2026-10-08 claims Arena values of Sol 6.1 Max Net Improvement +14.77% / median $1.10, Luna Max +2.77% / $0.10, and Astra Max +13.50% / $8.50. This uses another sample/configuration and displayed dollar metric; it is not comparable to quota credits. Net Improvement is not accuracy, and the rows do not establish causal quality-per-cost or speed. Max does not map to Ultra.

Recheck routing when model/effort availability, verified primary prices, credits, Fast multiplier, speed controls, or task-level end-to-end results change. Keep API USD pricing, quota credits, Fast multiplier, Arena metrics, and measured time separate. Unknown values remain unknown. Confirm before an additional purchase, pay-as-you-go use, plan/add-on change, another model’s Fast tier, or a charge beyond current approval; ordinary model/effort selection and authorized escalation continue without a new approval stop.

## Config evidence

Codex role/config schema is pinned to `9479e1fdb7396c3e3254f5e123489af829186893`, checked 2026-10-06: [discovery](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/agent-roles/src/discovery.rs), [loader](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/agent-roles/src/loader.rs), [role schema](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/agent-roles/src/agent_role_config.rs), [config schema](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/config/src/config_toml.rs). Auto-discovered roles need non-empty `name`, `description`, and `developer_instructions`; child-thread cap counts spawned children; `max_depth` is V1-only, so the no-child-spawn rule is explicit.

The separate `codex-cli 0.159.0-alpha.3` bundled catalog check from 2026-10-06 did not list `gpt-6.1-sol`; the current session tool catalog does. These are separate runtime surfaces; no alias or fallback is inferred. Previous CLI strict-config/role-load probes stopped at read-only initialization, so project-role loading in that CLI remains unverified. TOML validation does not prove a future runtime loaded or invoked a role. Check its catalog/loader before using the configuration there.
