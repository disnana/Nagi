# Task・Nagi 0.1.11とagent構成の引継ぎ

更新: 2026-10-07。[Nagi 0.1.11](https://github.com/disnana/Nagi/releases/tag/nagi-v0.1.11)は正式公開済み。Task S1/S2、条件付き消費の修正、agent構成、版PRはmain反映済み。公開sourceの4 OS CI、tag/latest、全8公開assetsのdownload/checksum、実Linux配布archiveのnative E2Eを確認した。既知のTask release blockerはない。旧Stage 1の未実装/停止指示は歴史的snapshotで、現在のsource状態ではない。

## 完成したsource

| PR | main merge | 範囲 |
|---|---|---|
| [#88](https://github.com/disnana/Nagi/pull/88) | `aee1987a7ede5eeebb5e253afd65cb5ede327dde` | S1 compiler/check/High/Low/public runtime、ownership・scope義務、native/費用/日英/独立review |
| [#89](https://github.com/disnana/Nagi/pull/89) | `f65c6093a3e2d53e0461551c89dd5fc24ab6ed1d` | AGENTSと6 custom roles、確認済みmodel ID、昇格・並列・review運用 |
| [#90](https://github.com/disnana/Nagi/pull/90) | `f9b25782a8704bcf962f689d9103ab804f7ab3cf` | 既存APIによるtyped monitor内側Err→親try、HTTP旧spawnの故障伝播維持 |
| [#92](https://github.com/disnana/Nagi/pull/92) | `97e62f82b1f67dbcea699071477cbeec7388a713` | 短絡RHS/lazy env fallbackのskip経路でも全Tの正常出口義務を維持 |
| [#91](https://github.com/disnana/Nagi/pull/91) | `003a594de086383100016b7c75466da37705646c` | 0.1.11版更新、日英migration/Docs、CHANGELOG/notes・配布準備 |

公開source `003a594de086383100016b7c75466da37705646c`はPR #91最終head `c2b227533b89268028ecb2d67e39492b7aa4e4a5`、local `64a7f3d5c5306abca579e5d95dd9990087747699`と共通tree `13b41a034bf463a9426c53327b42c4184a53c7ed`をAPI/gitで読戻した。compiler/runtime/CI/release-scriptは修正main #92との差分0。版差分はworkspaceとlockの自package2件だけ0.1.11、第三者依存とVSIX0.1.13は不変。引継ぎ本文へこの資料自身のcommit SHAは書かない。

Taskの採用意味論は[ADR 012](../adr/012-task-result-handles.md)、[S1結果](../task-handles-s1-results.md)、[S2結果](../task-handles-s2-results.md)、[条件付き消費修正](../task-conditional-consumption-fix.md)を参照する。全T一回consume/正常出口義務/escape拒否、sealed生成、業務Errとsticky故障、actual joinと同期Dropの責任、discardとclose/取消/rollbackの違いは決定済み。一般Futureや新しい公開cancel/closeを今回の完了条件から自動追加しない。

## 最終検証

最終PRの[checks 37553704154](https://github.com/disnana/Nagi/actions/runs/37553704154)はattempt2で成功、[website 37553703967](https://github.com/disnana/Nagi/actions/runs/37553703967)も成功。各4 OSの原ログから148/148契約、Task native8、runtime17、公開API3、doc9、両公開例×三構文、展開archive版0.1.11とTask gateをvalidatorで確認した。Linux95 result blocks・936成功・failed0・費用用ignored1、fuzz1000/seed305419896/panic0/bounded native16、両IDE/Ready成功。ignored、filter、skip、過去headを今回の成功実行へ数えない。

macOS Intel初回は手書きLow build中のcrates.io DNS/接続/config.json転送に失敗したinfra失敗。原ログを保存し、同sourceのfailed jobsだけ再実行した。テスト期待・timeout・並列・sourceは変更していない。成功済みjobが再実行されたとは数えない。Copilot quota通知はreview実行やapprovalではなく、独立Sol Max reviewで代用したという扱いにもせず、別に完了した独立reviewの根拠を保存する。変更要求と未解決threadは0。

独立最終Sol 6.1 Maxは最終凍結tree・public API/lifecycle・互換移行・版plan/notes/standalone runtime/4archive名、production79/artifact221 hashを確認し未解決0。Docs表現1文とREADME hash1行は修正・再読戻しした。reviewerの今回実行と継承したnative/全workspace/4 OSは報告で区別する。[最終原ログと公開確認](../../../benchmarks/results/task-release-0.1.11-published/README.md)に保存する。

公開sourceのmain [checks 37558874886](https://github.com/disnana/Nagi/actions/runs/37558874886)と[website 37558874635](https://github.com/disnana/Nagi/actions/runs/37558874635)は成功。各4 OSで148契約・native8・runtime17・public3・doc9・両公開例三構文・展開archive版/Task gateを原ログから再確認した。Linux workspaceだけの95 result blocksは936成功・failed0・費用ignored1、fuzz1000/panic0/bounded native16。mainのIDE/VSIX/Ready skipを再実行へ数えず、IDE成功は最終PRの結果として区別する。既存workflowのpublish job `112597346200`がtag→draft→8assets/checksum→readback→正式公開latestを完了した。手動tag/releaseは作っていない。

## 未実装・技術的負債と次PR候補

| 推奨順 | 内容 | 扱い・終了条件 |
|---|---|---|
| 1 | 同保証での保持/費用測定 | 大batch・長body・完了未join/未受取payload、HashMap high-water/関連causeを測る。multi-thread全allocation、他target、長時間、clean buildは現在未測定。必要な内部改善だけ小PRにする |
| 2 | 生成探索/corpus補強 | coverage-guided、match/loop・再生成/義務合流・故障順・取消drainを重点探索。縮小反例を恒久corpusへ。固定seed1000/有限native16をschedulerの網羅証明にしない |
| 3 | 小さな内部整理 | `task-contract-red`名称、ticket/native ID/receipt ledgerの整理。ownership/エラー/Drop/費用とHigh/Low/nativeの保証を維持する |
| 仕様判断が先 | 個別Task cancel/close/detach、scope外Task、一般Future保存 | 採用済みの残実装ではない。新公開言語仕様/互換APIを選ぶ具体案・検証計画を先に作る |
| 仕様判断が先 | 公開関連cause/acknowledge/recovery | 内部cause記録はある。sticky faultを消す公開APIは未採用で、今回のrelease blockerへ変えない |
| 別工程 | 公開SQLite Pool/Tx | [capacity判断](../sqlite-capacity-decision.md)・専用ADR/DB oracleを先に扱う。Task discard/取消をDB close/rollback完了としない |

S1/S2と#92で解決済みの機能を重複実装しない。保証の限界（同期Dropはabort要求まで、non-yielding強制停止なし、外部副作用rollbackなし、任意Rust Drop/panic payloadの普遍回復なし、外部Context cycleは利用者責務）は負債として自動拡張しない。

## agent運用

[AGENTS](../../../AGENTS.md)が起動・責任・昇格・並列・独立review方針を持ち、`.codex/config.toml`と6 role TOMLが実設定を持つ。Fast Luna Max→Sol 6.1 High（Engineer/通常Reviewerは同tier相互排他）→xHigh Architect→Max Critical Reviewer→Astra必要時だけ。通常は親含め3、child cap2、同model/effort1、Astra全effort1、nested agent禁止。短い作業は親が処理する。

schemaとモデル確認・CLIの未確認範囲は[agent-routing](../agent-routing.md)にある。workspace APIのSol 6.1は実High/Max起動で確認。standalone CLI role loaderはread-only runtime state初期化で停止し未実証、近いmodel IDへ暗黙fallbackしない。今回Maxを使った理由はrelease直前の独立最終レビューで、通常のasync/ownership修正はHighが担当した。

## 公開後の確認と次の担当

正式Releaseは`nagi-v0.1.11`、draft/prerelease=false、latest、`published_at=2026-10-07T02:11:53Z`。tagとtargetは公開source `003a594de086383100016b7c75466da37705646c`に一致する。notesは公開commitのCHANGELOGと前版0.1.10のtag/commit比較を持つ。

公開8assetsを実downloadし、byte数・SHA-256・GitHub digestと4sidecarを照合した。4archiveの`release.json`は同じsource/版/各platform、同梱standalone runtimeは0.1.11。binary/cacheは保存artifactへ入れない。公開Linux archiveをcheckout外から既存`verify.py`へ通しexit0。Task三構文/12拒否、受取/discard/内側業務Err、SQL、actor、local Rust依存、JSON/alias/元位置を確認した。依存cacheを再利用した26.849秒の実行でありclean buildではない。macOS/Windowsの公開assetsはdownload/identity/hashを確認し、実行確認は公開mainの各OS archive gateとして区別する。

この文書追記でcompiler/runtime/CI/版/tag/assetsを変更しない。次の担当は最新mainと公開tagを読戻し、上表の測定・生成探索・内部整理を独立PRで進める。新公開意味論を必要とする候補は具体案を先に判断する。S1/S2の再実装や0.1.11の再公開を行わない。
