# Codex agent routing and verification

2026-10-11 JST. [AGENTS.md](../../AGENTS.md) sets the task policy; [.codex/config.toml](../../.codex/config.toml) caps child threads. [Light Worker](../../.codex/agents/light_worker.toml), [Fast Worker](../../.codex/agents/fast_worker.toml), [Balanced Engineer](../../.codex/agents/balanced_engineer.toml), [Deep Engineer](../../.codex/agents/deep_engineer.toml), [Independent Reviewer](../../.codex/agents/independent_reviewer.toml), [Architect](../../.codex/agents/architect.toml), [Critical Reviewer](../../.codex/agents/critical_reviewer.toml), and [Astra Final Verifier](../../.codex/agents/astra_final_verifier.toml) define the role settings.

## Route by task

Choose the lowest-cost route likely to meet required correctness and audit evidence. Include model use, test/CI execution and waiting, failed attempts, rework, context transfer, and review effort in end-to-end time/cost. Rates alone do not show whole-task cost; subjective speed is not a measurement.

| Work | Default | Move up when |
|---|---|---|
| Few-line change | Parent does it directly | An independent deliverable appears |
| Bounded docs, rename, read-only question | Luna Medium / Light Worker | Scope or contract decision expands |
| Clear feature, fixture, ordinary test, obvious CI cause | Luna Max / Fast Worker | Same-cause targeted fix fails twice or code/contract boundary is unclear |
| Clear multi-file implementation or investigation where Luna rework may dominate | GPT-6.1 Sol High / Balanced Engineer | Unknown cause or important semantics need xHigh design review; final important-code audit uses a separate Sol 6.1 High agent |
| Important or difficult code; multi-module investigation | Sol 6.1 High / Deep Engineer | Unresolved semantics, compatibility, race, or lifecycle design needs Architect |
| Auth, authorization, secrets, integrity, concurrency, destructive change | Sol 6.1 High confirms approved design before implementation; xHigh only for unresolved semantics/race/alternatives; Sol 6.1 High implements | Independent Sol 6.1 High audit after checks; fix, rerun affected regressions, and re-review |
| Independent final audit on important code | Sol 6.1 High / Independent Reviewer, after checks | Fix findings, rerun affected regressions, and return final diff/results for re-review |
| Exceptional unresolved high-cost issue | Sol 6.1 Max; Astra Max only if Sol Max is insufficient | Do not escalate without a concrete question |

For high-risk work, Sol 6.1 confirms the approved design and failure/regression cases first; xHigh Architect is reserved for unresolved design. The independent reviewer then compares final diff, specification, results, and relevant regressions. Fixes require targeted reruns and a final re-review. Source-only review is not a substitute for execution.

2026-10-13 JST: At the user’s direction, remove GPT-6 Sol from routine or planned assignments and move ongoing/planned GPT-6 Sol work to GPT-6.1 Sol. Balanced Engineer is GPT-6.1 Sol High; Light/Fast remain Luna Medium/Max. High is the routine ceiling; xHigh is for unresolved important design, Max/Astra exceptional. No GPT-6 Sol Fast or routine GPT-6.1 Sol Max. Current prices/speed remain unknown.

For compiler changes, distinguish High/Low parse, checker outcome, negative reason and primary line, emitted Rust build, and a minimal native test where emission/runtime is affected. Mark blocked stages unverified and keep exact-head CI as a completion condition. When available, record model time, engineering time, test/CI runtime, CI wait, and rework separately; unavailable values stay unknown. Reuse results for the same source/head and rerun only for a relevant change or new evidence.

The 2026-10-11 cost/speed impression and supplied prices, plus the 2026-10-08 benchmark below, are historical evidence only. The impression is subjective, not measured; no latest price, speed, or total-cost claim is implied. GPT-6 Sol is no longer a routing option. Existing extra-charge and Fast permissions are unchanged.

## Observations and joint reassessment

This document is the separate record linked from AGENTS.md; do not create a file per routing decision. Add a short representative observation at a milestone or a consequential failure. Retain useful contrasting cases, not full conversations or secrets. Record task class and exact source/HEAD, model/effort and whether speed controls exist, instructions/context/tools/environment, measured task time, test/tool runtime, CI runtime and queue/wait, revision cycles, independent findings and executed validation stages. Model-only time, token use, credits and API USD stay unknown unless directly observable. Nested timings are not additive: a test duration can be part of the overall task interval.

Consider unclear instructions, incomplete context, scope, stale sources, fixture/test omissions, cache state, resource/network limits, task difficulty and differences in review depth before attributing a result to the model. A local test pass does not establish whole-PR readiness. Prefer a conservative known route while evidence is weak; change one routing or instruction variable at a time when practical during necessary work. Do not rerun the same task on several models just to manufacture a benchmark. Reassess on repeated rework, a material miss, changed availability/pricing or several comparable task observations; do not wait for a quota before escalating a concrete risk.

For a material routing-policy change, give Luna and Sol 6.1 the same evidence and question independently, then exchange their arguments once. Each can disagree on equal terms; model prestige is not a tie-breaker. The parent records adopted/rejected/unresolved proposals and reasons. Agreement between models does not prove correctness or replace tests or independent code audit. Routine dispatch uses the defaults without a meeting. Keep independent development moving within the concurrency cap; only the designated owner edits shared policy files.

### 2026-10-11 historical observations and decision

| Evidence | Observation | Limits and action |
|---|---|---|
| User report on GPT-6 Sol Max | Favorable cost/speed balance in their experience | Subjective, no controlled latency or billing data. Consider it for clear bounded implementation; do not rank all models from this report |
| PR [#115](https://github.com/disnana/Nagi/pull/115), source `070de14b40218a20ce267b93a2aac928aad4d535`, bounded two-file CI correction | GPT-6 Sol Max task interval reported as 00:33:30–00:38:40 UTC (5m10s); targeted Python run observed 60 passes in 6.502s, included in that interval; 22 pinned dependency blobs checked | Task interval includes tool/test/reasoning work and inherited investigation, not model-only latency. Speed setting unavailable; tokens, credits and USD unknown. Rust/4-OS execution and independent final audit were pending at this observation. One case does not demonstrate superiority |
| PR [#114](https://github.com/disnana/Nagi/pull/114), source `0dd9cf1c651efb3a99c794ccdd516516f7a9374e`, [CI 38098161525](https://github.com/disnana/Nagi/actions/runs/38098161525) | Focused local Session tests passed, but CI found duplicate test-module loading and missing SessionSameSite constants in the independent inventory | Integration coverage was incomplete; broader local checks were also limited by disk capacity. Do not infer a model cause. Add the relevant inventory and harness checks to the next targeted validation; preserve the independent oracle |

At that time, Luna Max and Sol 6.1 High independently recommended bounded GPT-6 Sol Max use, preserving design/audit gates and distinguishing subjective speed from measurement. After exchanging arguments, both supported a short record in this existing document and opposed per-task meetings or prestige-based ranking. The parent adopted those proposals then. Blanket “Sol Max is cheaper/faster”, escalation by file count alone and AI consensus as quality evidence were rejected. Comparative completion cost/latency remained unresolved. The 2026-10-13 user instruction supersedes the historical GPT-6 Sol route; earlier observations remain records, not current guidance.

## Ready for review

The parent removes Draft after the PR's accepted scope is complete and the latest HEAD has passed applicable mandatory CI, regressions, independent review and review after fixes. Recheck HEAD/base, review coverage, unresolved findings, mergeability, dependencies and any required GUI/user acceptance immediately before the action. Incomplete stacked dependencies and explicit PR holds keep Draft. This operation is authorized by the user's standing instruction; do not ask again after the conditions are met. Verify GitHub reports `draft=false`, then report the PR, HEAD and evidence. Unknown mergeability prevents the transition. A later HEAD change or regression invalidates readiness; return the PR to Draft when the tool permits and record why. Do not claim the operation happened if the tool is unavailable. Ready status grants no permission to merge, bump versions, tag or publish.

## Concurrency and escalation

`.codex/config.toml` permits two child threads plus the parent (at most three agents); children do not spawn children. Spawn only for independent work with a defined result. Only one agent uses a given model ID plus reasoning effort at a time. Balanced Engineer, Deep Engineer and Independent Reviewer all use GPT-6.1 Sol High and therefore run sequentially.

Luna Medium → Luna Max → GPT-6.1 Sol High → GPT-6.1 Sol xHigh → GPT-6.1 Sol Max → Astra is a possible path, not a required ladder. Start higher when risk warrants it. After two targeted failures from the same cause, or sooner on a specific contract/security uncertainty, stop repeating and pass the exact input, command, diagnostic, source line, and prior results forward. Do not blame the model without evidence. Reassess a route if time/quality/total cost fails to improve.

## Active catalog and service tiers

The session tool catalog lists `gpt-6-luna` Medium/Max, `gpt-6-sol` Max, `gpt-6.1-sol` High/xHigh/Max, and `gpt-6-astra` Max. No role routes to `gpt-6-sol`. Role TOML stores model/effort; the invocation interface has no speed-tier option. “Fast Worker” is a role name. Only Luna Max Fast is explicitly approved when offered; other Fast tiers, new purchases/plans or charges beyond approval require confirmation. Ordinary model/effort selection and Sol 6.1 escalation proceed without approval if they add no charge. Do not add unsupported tier keys. Fast’s displayed multiplier and `max` effort do not promise speed.

## Historical usage-credit evidence and limits

The user-provided compiled report dated 2026-10-11 claims “Business/Enterprise” credits per 1M tokens, ordered input / cached input / output. It is not a verified provider billing/model-price UI; its issuing product and primary URL were not verified. These values are not official API USD prices.

| Report label | Tier | Credits per 1M (input / cached / output) |
|---|---|---:|
| GPT-6 Luna | Standard / Fast | 2.5 / 0.25 / 12.5; 5 / 0.5 / 25 |
| GPT-6.1 Sol | Standard / Fast | 50 / 2.5 / 250; 100 / 5 / 500 |
| GPT-6 Astra | Standard / Fast | 250 / 25 / 1,250; 500 / 50 / 2,500 |
| GPT-6 Sol | Standard / Fast | 50 / 5 / 250; 100 / 10 / 500 |
| GPT-5.6 Sol | Standard; Fast if offered | 100 / 10 / 500; 2× Standard multiplier shown, exact Fast rates unverified |
| GPT-5.6 Terra (report label only) | Standard | 50 / 5 / 300 |
| GPT-5.6 Luna | Standard | 5 / 0.5 / 30 |

The report’s Fast rows claim 2× usage credits for listed models, not 2× measured speed. API USD rates, total session bills, actual latency, and per-task costs were unavailable. GPT-5.6 Sol, GPT-5.6 Terra, and GPT-5.6 Luna are historical report labels only; no configured agent role uses those models. Non-role rows above are observations only; verify the active ID/effort before routing to any model.

A separate user-provided compiled report dated 2026-10-08 claims Arena values of Sol 6.1 Max Net Improvement +14.77% / median $1.10, Luna Max +2.77% / $0.10, and Astra Max +13.50% / $8.50. This uses another sample/configuration and displayed dollar metric; it is not comparable to quota credits. Net Improvement is not accuracy, and the rows do not establish causal quality-per-cost or speed. Max does not map to Ultra.

Recheck routing when model/effort availability, verified primary prices, credits, Fast multiplier, speed controls, or task-level end-to-end results change. Keep API USD pricing, quota credits, Fast multiplier, Arena metrics, and measured time separate. Unknown values remain unknown. Confirm before an additional purchase, pay-as-you-go use, plan/add-on change, another model’s Fast tier, or a charge beyond current approval; ordinary model/effort selection and authorized escalation continue without a new approval stop.

## Config evidence

Codex role/config schema is pinned to `9479e1fdb7396c3e3254f5e123489af829186893`, checked 2026-10-06: [discovery](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/agent-roles/src/discovery.rs), [loader](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/agent-roles/src/loader.rs), [role schema](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/agent-roles/src/agent_role_config.rs), [config schema](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/config/src/config_toml.rs). Auto-discovered roles need non-empty `name`, `description`, and `developer_instructions`; child-thread cap counts spawned children; `max_depth` is V1-only, so the no-child-spawn rule is explicit.

The separate `codex-cli 0.159.0-alpha.3` bundled catalog check from 2026-10-06 did not list `gpt-6.1-sol`; the current session tool catalog does. These are separate runtime surfaces; no alias or fallback is inferred. Previous CLI strict-config/role-load probes stopped at read-only initialization, so project-role loading in that CLI remains unverified. TOML validation does not prove a future runtime loaded or invoked a role. Check its catalog/loader before using the configuration there.

## 2026-10-20: 開発ビルド基盤の担当記録

base `2c703b89645b164f586a926e473d08395be65c90`。当sessionのspawn catalogにあるIDを使用。Luna Mediumは読取専用の標準API対応調査、Sol 6.1 Highは既存世代契約とcache整合性の実装前確認、Luna Maxは独立した利用者CI generator、別のSol 6.1 Highは実装・対象検査後の最終auditを担当した。子からのspawnと同じfileの並行編集はなし。親を含む同時稼働は最大3体、同じmodel/effortの子は順番に実行した。service-tier指定interface、model別使用量・料金・実model時間は観測できず、Fast速度や費用削減を主張しない。

CLIの公開0.2.0と開発flagの差、生成workflowのtoolchain指定、path記録とcompiler digestの限界を修正・明記し、独立auditで再確認した。独立auditは実Rust/native/full-suite/CIの代わりとせず、実行した範囲は[計測記録](../../benchmarks/results/smart-build-2026-10-20/README.md)へ分離する。新しい生成cacheによるnative再利用は有効にしていない。

## 2026-10-21: 生成本体の再利用

main base `1386411`、Rust実装`5a90083`、PR #140。Sol 6.1 Highを実装前の整合性・世代境界のread-only確認に使い、親がcompiler/generation/testsと実計測を担当した。独立したPython receipt v2と配布gateはLuna Maxが担当し、親reviewで汎用TOML風parserを固定された生成manifestのtable/key検査へ縮小した。親が実Cargoで見つけたworkspace自動membership、Windows表記の診断、projectionからCASへのhardlink aliasは契約を維持して修正し、元assertを緩めていない。

子の追加spawn、同じfileの並行編集、同model/effortの並行子はなし。実行検証後の最終auditは別のSol 6.1 Highへ分離する。service-tier指定interface、model別使用量・料金は不明。今回の[build計測](../../benchmarks/results/generated-body-reuse-2026-10-21/README.md)はmodel速度やCIコストの測定ではない。最終audit/CI状態はPR本文・Checksで読み戻す。

## 2026-10-21: 関数依存単位への拡張

base `0e9a575`、core `39dc51d`。Sol 6.1 Highで実装前の整合性・Cargo単位境界とmixed facade案を確認し、親がcompiler/CAS/generation/native回帰を実装した。schema 3の閉じたmanifest grammar/依存閉包/Windows pathは契約判断を伴うため、同じHigh担当へ独立fileの実装を分離した。Luna Maxは仕様が固まったbenchmark harnessのmode/fixture/repeated editsだけを担当し、実計測は親が他のローカルbuildを止めて行った。Highによるmixed fixtureの修正は、最小runtimeのSerde不足と禁止fn fieldというfixture不備を言語契約へ戻す作業であり、成功までassertや期限を緩めていない。canonical shadow例のenum比較も非対応のため既存matchへ修正した。

親を含め最大3体、子spawnと同じfileの同時編集なし。同model/effortの子は順番に動かし、実装担当とは別のSol 6.1 Highに必要検証後の最終auditを渡す。native/workspace/配布/CIとauditは別証拠である。役割ごとの実model時間・使用量・費用・service-tier設定interfaceは不明。報告した7%前後のbuild差はmodel速度やCI費用の差ではない。

## 2026-10-22: PyCharm編集補助の適用

base `5d3e58e`。Luna MaxへPython/Nagiのsource比較と、既存VS Code実装を根拠としたbounded typed補正の独立fileを担当させ、親がEnter履歴・実IDE検証・Docsを担当した。担当範囲を分離し、子の追加spawnなし。親reviewでtoken型、immutable Listのsort、非先頭closer、設定幅の上限を修正し、実fixtureでは選択解除のphaseとUndo/Redoによるstamp復元をRED/GREENで確認した。modelだけを原因とせず、SDKのcallback順序とsource/fixture不備を区別した。参照URLの親の推測5件も実sourceと照合して修正した。

ローカルはsystem JREにCLI javacがなく、SDK付属JBRを実toolchainとして指定した。Gradle/SDK初回準備は約9分、cache準備後の全186件とZIP生成は72秒（IDE背景接続警告あり）。これらはmodel時間やNagi利用者のbuild速度比較ではない。model使用量・料金・service-tier指定は未計測/未対応。実装検証後の独立Sol 6.1 High監査とexact-head CIの結果はPR本文で別記する。

Enter補助の独立Sol 6.1 High監査で、SDK `DocumentEx.getModificationSequence()` default恒常0を有効な履歴と扱うP2を検出した。親が実actionを保ったdelegate/document proxy回帰を追加し、2件中1件RED（16秒）を確認した。正値sequenceだけ記録・照合し、0開始の実sequenceは初回Enterから有効にする。監査対象旧HEAD `6ed5520`と修正後HEADの証拠を分け、更新差分・実行結果の再reviewを依頼する。
修正後は実compiler連携を含む全188件/失敗・skip 0とZIP生成が55秒で成功し、descriptor/hash検証を通した。これはwarm SDK条件の実行時間で、変更前比の高速化は主張しない。

2026-10-22 Tokio #8692対応では、親がupstream PR/release・tracked locks・生成Cargo境界を調査し、独立Sol 6.1 Highのread-only事前調査がruntime最低版/standalone fixture整合と必須failure casesを確認した。実装はmanifest/lockと日英文書に限定し、cache codeや履歴benchmarkを変えない。focused runtime862成功/計測1ignored、生成appのfresh/1.54.0/1.53.1 lock更新と旧exe保持を実行した。実装後の別のfresh Sol 6.1 High最終監査とexact-head CIはPRで追跡する。model時間/使用量/料金は未計測。ローカルSDK cache削除は初回自動reviewが拒否した後、ユーザーの明示許可により前taskで取得した再download可能なIDE SDK約2.9GBだけを削除し、ZIP/raw logとNagi app cacheは保持した。


## 2026-10-22: JetBrains documentationの独立差分

base `765f0b0`。Sol 6.1 High担当がread-onlyでhover/diagnostic/compiler JSONとsnapshot境界を調査し、親の設計採用後、専用worktreeでplugin実装・意味のあるnegative/実compiler/JVM表示testsを担当した。compiler/CIは変更せず、子の追加spawnなし。親はSDK bootstrapと別PRのQA・後続PR操作を担当し、Gradleは同じcacheを使うため単独実行した。必要な検証後の独立最終auditは別担当へ渡す。

全214件と追加HTML renderer1件を確認し、原文保持・HTML表示・exact location接続を実fixtureで検証した。SDK準備中に編集開始したbootstrapは厳密なmain baselineと数えない。packagingのread-only data pathとVerifierのread-only home/remote接続失敗をsource不具合と分け、原ログを保持して専有設定だけを修正する。Verifier/他IDE CI/実GUIの未完gateはPRに残す。[詳細記録](jetbrains-documentation-plan.md)。モデル別時間・使用量・料金・service-tierは未計測で、テスト時間をmodel速度の比較には使わない。
## 2026-10-22: 0.2.1の重複処理と計測基盤

base `e2d7280`。Sol 6.1 Highの事前調査でparser token再利用、既存unitsの境界と将来のasync段階案を確認し、親がparser/sourceと回帰・実計測を担当した。CI担当Luna Maxの再開はcapacityエラーで実行できず、同じ証拠と限定file範囲を既存High担当へ渡した。モデル品質の失敗とは扱わず、同じ探索は繰り返さない。HighがCIの未使用release build除去・fixture cache共有・exact inverse検査を実装し、その後に独立fileの8ケースmatrix harnessとpure safety testsを担当した。親を含め最大3体、子spawnと同じfileの並行編集はなし。実行後の最終auditは実装者と別のfresh Highへ分離する。

全workspace検査の初回はsandboxがsocket bindを拒否したため、source/期限/assertを変えずnetwork許可を付けて成功した。670.09秒はvalidationの実測で、before/after速度差やmodel速度ではない。compiler比較では別checkout間のCargo target共有で同じbinaryが残る観測を除外し、candidateを再compileしてSHAを確認した。原因未確定の環境・cache観測をNagi CASの脆弱性やmodel不良と断定しない。ユーザーの追加明示許可に基づき、区切りで両filesystemの容量確認と専有一時targetの整理を行い、source本体・未commit差分・raw log・比較CLI・旧exeを保持した。実model時間・使用量・料金・service tier設定は未観測。実行結果と独立audit/CIは[記録](../../benchmarks/results/release-0.2.1-build-stability-2026-10-22/README.md)とPRへ分離する。


## 2026-10-22: #147の失敗診断counter保持

Sol 6.1 High担当がread-onlyで保存済みCI事実とbrowser Handler境界、既存counter parserを調査し、親が最小Python補助出力の設計を承認した。同担当が合成RED→GREENとprivacy/owner/bounds回帰を実装し、親は事前固定のstock/diagnostic各3run比較を別に担当した。全run成功・未再現を原因解決と混同せず、runtime/期限/assert/parallelismを変更しない。親reviewのwire assertion終端を反映し、同ownerの両iteration拒否もRED→GREENで確認した。

Session65/SF07 summary37/native-finish20/source guard16/CI policy160件が成功。実装後の最終auditは別のSol 6.1 Highへ分離し、子の追加spawnなし。新HEAD CIと1000ms原因の継続調査はPRへ残す。model時間・使用量・費用・service tierは未計測。Python検査時間やcompile込み比較時間をmodel性能の根拠には使わない。


2026-10-22、#148候補の入力時色消失をSol 6.1 Highが事前承認済みpresentation-only設計で実装。実ExternalToolPassのRED→GREEN、debounce起動/取消数、marker上限/取消cleanup、trust復帰の初回full失敗と修正を記録した。modernDocumentationTarget testのJob context不足はSDK bridgeをArchitect xhighがread-only確認し、正規indicator/coroutine/blocking/read contextでwarm成功。全225件71秒成功。病的な同位置20,000marker fixtureは停止し、製品上限を維持したtest-only境界注入へ変更。compiler/CI/run profile/coldasyncの製品変更は行わず、別Highの最終auditと4IDE CI/実GUIを残す。親と担当のGradle共有cacheを直列使用し、workspace空き4.4GiB/tmp7.9GiBを維持。利用量・モデル単体時間は未測定。


2026-10-22、Sol 6.1 Highが#148のcold hover/localsを、前段colorの別High audit後に実装。Architect xhighはscalar通知・timer所有・read→lock順・限定recapture責任移譲を事前/境界発見時にread-only確認した。取消testの同assert2失敗で停止し、SDK ApplicationImplのPCE→null wrapperを根拠にtest executorを是正、assertを維持してactual SDK Deferred取消を検証。公開0.2.0と開発版のimport文法差、CRLF/Low commentのfixture不備を製品bugと区別。14ticket検査/公開CLI連携と全243件84秒成功、dispose後trust eventのRED→guard→targeted16秒成功後に最終全体/ZIP/Verifierを実行する。新run profile/compiler変更なし。別High最終audit/4IDE CI/実GUI受入は別gate、model単体時間と利用量は未測定。


cold最終guard後の全243件77秒・ZIP21秒・IC251 Verifier11秒Compatible（deprecated3/experimental12）を実行し、source/test/docsを凍結。API警告を隠さず最新4IDE CIへ渡し、別Highの独立最終auditと実GUIを未完gateとして維持する。


2026-10-22、Sol 6.1 Highが1792138から専有worktreeでJetBrains Development/Release Runを実装。事前read-only designを親が採用し、in-flight共有を省いて各Runの独立bounded probeと完了cacheへ限定した。Luna Medium/親のconsoleログ・SDK262調査で同期callback中のenvironment破棄を絞り、Highが実registered runnerでRED、Architect xhighが単一CAS＋project-only temporary process fallbackを事前確認してGREENにした。全266件83秒後、割込みcleanupの対象回帰18秒を経て最終267件84秒成功。正式CI候補0.2.1のnativeはcold条件非対称を明記し、harness誤期待と正規管理run拒否を製品不具合と区別した。probe実行数・cache再利用・独立同時miss・取消/trust/settings/dispose/identity交換を検証し、native所有targetだけ整理。別High auditは実行完了後、model単体時間・利用量・費用は未測定。子の追加spawnなし、他worktree/本体CLI/CIは変更しない。


上記Run候補は最終267件/ZIP21秒/保持ZIPのVerifier10秒Compatible（deprecated3/experimental17）/CI policy160件まで実行。専有native targetは証拠保護後に削除し、全実行終了・source/test/docs凍結後に親へ所有権を返す。別High audit・最新HEAD CI・Windows GUIは未実施gateとして分離する。

2026-10-22、Sol 6.1 Highが専有worktree（base f75300a）でSupervisor terminal eventのP1修正を担当。親/Architect xhighが停止intentとpublicationのrace、Release/Acquire barrier、cleanup後のpollを固定するfinite gateを事前承認した。Highはhookだけの旧実装でSHUTDOWN0/STOPPED1のREDを再現し、gate解除・thread join・drainを済ませた最後のassertで観測。製品4行＋説明2行を追加しGREEN、actor45件・全runtime923 passed/3既存ignored・clippy/fmt・High/saved Lowの実nativeを実行した。1回のWindows複合assert失敗（実数なし）との根因同一性は主張しない。test期限/assert/並列度を維持、追加spawn/commit/pushなし。source/test凍結後に別Highが最終auditを行い、その後改訂4OS CIへ進む。専有targetのCargo再利用とaudit後整理は親へ引き継ぎ、元cache/CLI/evidenceは保護する。model単体時間・使用量・費用は未測定。


2026-10-22、#149 SF08 prerequisitesをSol 6.1 Highが限定調査。親がsource pin・両APT argv限定・閉鎖後scalar公開の事前設計を承認し、同Highが実装とRED→GREEN/Python64/Node148/source guard16/policy160を実行した。managed transport設定差と不要NodeSource依存をprivate診断で分離し、GitHub240秒失敗の原因同一性は未確認とした。原trustのfresh fullはAPT通過後build101/unknown・browser0、受入成功とは扱わない。所有container/imageのみ区切りで整理し、private証拠と原ログを保持する。新HEAD CI・別High最終auditへ引継ぐ。追加spawn/commit/pushなし、model時間・費用は未測定。詳細は[sf08-prerequisite-repositories.md](sf08-prerequisite-repositories.md)。


2026-10-23、#148 d06のSF08 install236秒timeoutをSol 6.1 Highがread-only調査。親承認後、専有worktreeで固定2名bounded captureとfailure-only status enum/数値だけを実装した。pinned image内APT2.8.3を実測し、当初2.7.14参照を訂正して正確なUbuntu source archiveも確認。native1回30.362秒/閉鎖ECHILD、Python70/Node148/guard16/policy160、RED→GREENを実行し、原trustと240秒/security/browser gateを維持した。証拠保存後に専有Docker objectsだけ整理。根因未解決・観測追加として凍結し、親が別High最終監査とexact-head CIへ進める。追加spawn/commit/pushなし、model時間/費用は未計測。[詳細](sf08-install-status-diagnostic.md)。
