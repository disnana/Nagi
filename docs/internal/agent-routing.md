# Codex agent routing and verification

2026-10-11 JST. [AGENTS.md](../../AGENTS.md) sets the task policy; [.codex/config.toml](../../.codex/config.toml) caps child threads. [Light Worker](../../.codex/agents/light_worker.toml), [Fast Worker](../../.codex/agents/fast_worker.toml), [Deep Engineer](../../.codex/agents/deep_engineer.toml), [Independent Reviewer](../../.codex/agents/independent_reviewer.toml), [Architect](../../.codex/agents/architect.toml), [Critical Reviewer](../../.codex/agents/critical_reviewer.toml), and [Astra Final Verifier](../../.codex/agents/astra_final_verifier.toml) define the role settings.

## Route by task

Choose the lowest-cost route likely to meet required correctness and audit evidence. Include model use, test/CI execution and waiting, failed attempts, rework, context transfer, and review effort in end-to-end time/cost. Rates alone do not show whole-task cost; subjective speed is not a measurement.

| Work | Default | Move up when |
|---|---|---|
| Few-line change | Parent does it directly | An independent deliverable appears |
| Bounded docs, rename, read-only question | Luna Medium / Light Worker | Scope or contract decision expands |
| Clear feature, fixture, ordinary test, obvious CI cause | Luna Max / Fast Worker | Same-cause targeted fix fails twice or code/contract boundary is unclear |
| Important or difficult code; multi-module investigation | Sol 6.1 High / Deep Engineer | Unresolved semantics, compatibility, race, or lifecycle design needs Architect |
| Auth, authorization, secrets, integrity, concurrency, destructive change | Sol 6.1 High confirms approved design before implementation; xHigh only for unresolved semantics/race/alternatives; Sol 6.1 High implements | Independent Sol 6.1 High audit after checks; fix, rerun affected regressions, and re-review |
| Independent final audit on important code | Sol 6.1 High / Independent Reviewer, after checks | Fix findings, rerun affected regressions, and return final diff/results for re-review |
| Exceptional unresolved high-cost issue | Sol 6.1 Max; Astra Max only if Sol Max is insufficient | Do not escalate without a concrete question |

For high-risk work, Sol 6.1 confirms the approved design and failure/regression cases first; xHigh Architect is reserved for unresolved design. The independent reviewer then compares final diff, specification, results, and relevant regressions. Fixes require targeted reruns and a final re-review. Source-only review is not a substitute for execution.

For compiler changes, distinguish High/Low parse, checker outcome, negative reason and primary line, emitted Rust build, and a minimal native test where emission/runtime is affected. Mark blocked stages unverified and keep exact-head CI as a completion condition. When available, record model time, engineering time, test/CI runtime, CI wait, and rework separately; unavailable values stay unknown. Reuse results for the same source/head and rerun only for a relevant change or new evidence.

## Concurrency and escalation

`.codex/config.toml` permits two child threads plus the parent (at most three agents); children do not spawn children. Spawn only for independent work with a defined result. Only one agent uses a given model ID plus reasoning effort at a time. Deep Engineer and Independent Reviewer both use Sol 6.1 High and therefore run sequentially.

Luna Medium → Luna Max → Sol High → Sol xHigh → Sol Max → Astra is a possible path, not a required ladder. Start higher when risk warrants it. After two targeted failures from the same cause, or sooner on a specific contract/security uncertainty, stop repeating and pass the exact input, command, diagnostic, source line, and prior results forward. Do not blame the model without evidence. Reassess a route if time/quality/total cost fails to improve.

## Active catalog and service tiers

The current session agent tool catalog exposes `gpt-6-luna` Medium/Max, `gpt-6.1-sol` High/xHigh/Max, and `gpt-6-astra` Max. Role TOML specifies model and reasoning effort; the current invocation interface has no speed/service-tier option. “Fast Worker” is a role name. Existing explicit approval covers only Luna Max Fast when the environment offers it. Fast for other models, additional purchase/pay-as-you-go/plan/add-on, or another charge beyond current approval requires confirmation. Ordinary task-driven model/effort selection and Sol escalation within current authorization should proceed without another approval stop. Do not add an unsupported service-tier key. Fast’s displayed usage multiplier and `max` reasoning effort do not promise a speed increase.

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
