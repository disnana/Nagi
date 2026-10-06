# Task handles S1完成後と次リリースの引継ぎ

2026-10-06。PR #88はS1完成PR。compiler→公開runtime、90/90契約、三構文native、4 OS、独立レビュー、費用、日英Docsは接続時に完了した。最終レビューで110契約へ補強し、文書不一致とservice専用oracle不足を修正する。[最終レビュー](../task-handles-s1-final-review.md)と[接続結果・原ログ](../task-handles-s1-results.md)を先に読む。

最新のユーザー指示は同じセッションでTask関連を次Nagiリリース公開まで継続するもの。S1完成だけで停止せず、別意味論/大きい機能はPRを分け、依存順に進める。既存手順から次版が決まりrelease blockerがなければ、必要PRのmerge・version bump・tag/releaseを進める承認がある。前の「merge/release/版更新禁止」はその当時の履歴である。未採用の公開言語仕様・互換性のないAPI案・breaking判断・DESIGN矛盾・重大リスクは具体案を作って確認する。

rootの[AGENTS](../../../AGENTS.md)、[ADR012](../adr/012-task-result-handles.md)、[invariants](../language-invariants.md)、[pipeline](../compiler-pipeline.md)を読む。[Stage 1引継ぎ](2026-10-06-task-bridge-stage1.md)はprivate bridgeで停止した履歴で、現在の「Task未実装」の根拠ではない。Nagiでアプリを書く資料は[ai/README](../../../ai/README.md)。

## 完成範囲・保持する判断

- SpawnBind/canonical std.task、ScopeId/binding義務、move転送、全T一回consume/正常出口awaitまたはdiscard、scope/引数/return/field/container/wrapper/他task escape拒否。
- High/保存Low/手書きLowの型・ownership一致とsealed生成。暗黙cloneやemitter所有権再推論なし。rustcのcrate/trait/Send/Sync責務は委譲する。
- business ResultとTaskFailureの二重Result、sticky primary/関連cause、兄弟取消要求→全actual join、legacy/body元Error保持。
- TaskFailureはopaque非Copy/Clone/shared、kindは四値Copy、messageはFailure-origin view。discardはunitで受取放棄。取消/Dropを終了・close・rollback完了に代用しない。
- 旧spawn-only ScopeとSupervisor/HTTP旧連携を維持。最寄りTask binding scopeだけTaskScope。nested scopeは独立。

Task静的制約はNagi checkerの保証。手書きRust handle consumeだけでmust-consume/escape禁止を代用しない。同期Drop、non-yielding、任意Rust Drop/panic、外部副作用の限界は機能を増やせば普遍保証できる負債ではない。

## 最新source・CIの確認

最終review読取基点はmain `9ba4a104be6f65dba61eda0c7b1ef0f98c892cf7`、PR #88 head `a9a0d42e94d6c7518bd7d955c9400299e554888c`、tree `f077a9b1df2d10484cfd8c06e42e82996d2289cc`。基点checks `37492925004`/website `37492924210`は成功。補強後headとmain反映は実GitHub値・tree・原ログを読戻す。この文書自身のcommitを本文へ自己参照させない。

補強後は110/110、Task native6群（全20positive対の三構文を含む）、runtime17、公開API3、runtime doc9。compiler/runtime本体と依存は接続source `34ac4d5` から不変。全workspace934成功、failed0、費用用ignored1、fmt/clippy、runner3、release unit98、検証用Linux archiveのTask三構文/12拒否と既存配布検査全体が成功した。Solの独立最終レビューはDocs三指摘の修正と同梱runtimeによるTask実行も確認した。新公開headの4 OS CIはこれから読み戻す。次の担当は最新main/PR/head/tree/working tree/未解決reviewを読み、後続の他人の変更を上書きしない。

## 未実装・次工程

| 項目 | 現在と扱い |
|---|---|
| S2 service接続 | PR #90で完成・main反映済み。既存try/Result/ErrorでSupervisor monitorの内側Resultを親body Errへ接続し、HTTP旧spawnを維持。新しい明示fault昇格operationは不要で追加していない |
| 旧spawn移行 | 通常業務Err、service終端Err、HTTP失敗を分類して具体serviceだけを移行。全spawnの機械置換は禁止。HTTPをtyped handleにしてdiscardすると内側Errが失われ得る |
| 個別Task cancel/close/detach、scope外Task、一般Future保存 | S1機能ではなく採用済み次工程でもない。今回のrelease条件の語句だけから追加しない |
| 公開関連cause/acknowledge/recovery | 内部記録以上の公開APIは未実装。sticky故障解除は別の意味論判断 |
| 公開SQLite Pool/Tx | 別工程。private試作/取得予算を公開Task APIと混同せず、[capacity課題](../sqlite-capacity-decision.md)を先に判断する |

全T義務・sticky fault・二重Resultは決定済みで再承認待ちへ戻さない。一般Result/owned must-use、shared actor message、独自VM/scheduler/GCを自動承認された変更と扱わない。

## 技術的負債・有限な検証

| 優先度 | 課題 | 根拠と次の行動 |
|---|---|---|
| P2・保持/性能 | 完了未join、join済み未受取payload、外部handle Drop後record、HashMap high-water容量、関連causeを保持し得る | [費用原ログ](../../../benchmarks/results/task-handles-s1-2026-10-06/README.md)にpayload1024×64B等。O(n²)故障receive掃除は修正済み。drain終端sweep/capacityは別に測り、必要な内部改善だけ行う |
| P2・測定範囲 | Linux/current-thread、同TaskScope、呼出しthread counter、warm依存。multi-thread総allocation/他target/clean build/長時間は未測定 | 同保証の手書き/生成Rustを測り、bare Tokioは保証が違う参考に分ける。一般速度優位/ゼロコストを主張しない |
| P2・探索範囲 | 固定seed native16経路、mutation1000のparse/check/Low中心。coverage-guided/任意schedulerの証明ではない | match/while・義務転送/再生成・nested scope・故障順・取消drainを重点生成し、反例を縮小してcorpusへ保存 |
| P3・内部整理 | `task-contract-red` 名称、設計の先行手順、ticket/native ID/receiptのrecord | rename/ledger簡素化は保証と費用を保つ小PR。古いsnapshot復元や0件GREENをしない |

これらはS1意味論の未解決ブロッカーではない。新capacity/timeout/observer/独自GCを負債対策として無断追加しない。

## 次PR候補・推奨順

| 順 | PR候補 | 成果と終了条件 |
|---|---|---|
| 完了 | #88の最終補強 | 110契約・三構文service・archive Task E2E・全回帰・4 OS・日英整合・独立最終レビューを完了し、main `aee1987`へ反映 |
| 完了 | #90 S2の具体service移行 | 既存APIでmonitor内側Err→親body Err、HTTP旧故障伝播を保持。業務reply継続、terminal停止、正常shutdown、親取消、context保持/cycleの限界を三構文・実socket・4 OSで確認し、main `f9b25782`へ反映 |
| 3 | 次リリース準備/公開 | API差分、#87 mandatory moveのmigration note、changelog/notes、version、配布artifact/hash、全suite/examples/HighLow/native/4 OSと独立最終レビュー。既存release workflowを使い重複tag/releaseしない |
| 後続 | 保持/測定/探索の改善 | 同条件大batch/長body/故障/Dropを測り、小さく改善。Task releaseの既知blockerと単なる範囲拡張を区別 |
| 別工程 | 公開Pool/Tx | S2 acceptanceとcapacity判断を前提に専用ADR/DB oracleで進む。Task取消/discardをDB cleanup完了としない |

最新のmerge済PR・残件・検証・release状態はこの資料と[progress](../progress.md)へ追記する。PR完成、CI成功、Docs完了だけでセッションを止めず、既知release blockerを解消して公開読戻しまで継続する。

## 追記: S1 mergeとS2の現在地

#88の最新head `08e90c6984e689f7d026b3ff40277b9898ab4c68`はchecks `37540011861`・website `37540011529`が成功し、4 OSの110契約・private/public native・配布Task三構文を読戻した。main `aee1987a7ede5eeebb5e253afd65cb5ede327dde`へmerge済みで、双方tree `bc62c787471d9e4481e36c0cca6e2ed294cebbc1`。上の未merge・CI待ち表記は最終補強時点の履歴である。

S2はPR #90で既存APIを使い完成し、サービスmonitorの内側Resultを親tryへ接続した。新故障昇格APIは不要と判断した。HTTP旧spawn、業務Err継続、同期Drop/context保持の限界を維持し、三構文×7nativeケースと公開両例三構文が成功。独立指摘3点の修正と読戻し、全回帰、4 OSを完了した。最終head `f149705308fb042f3633fcc0a6cc533579ce5c77`のchecks `37545273020`・website `37545272936`は成功、main `f9b25782a8704bcf962f689d9103ab804f7ab3cf`へmergeし、共通tree `84d1696053a9cf5ef256b44fcd50afff91863c9d`を読戻した。[S2結果](../task-handles-s2-results.md)が正本。リリース候補は既存patch履歴に沿う0.1.11、VSIX0.1.13は変更しない。migration/API差分とCI原ログは[release artifact](../../../benchmarks/results/task-release-0.1.11/README.md)へ保存し、[release引継ぎ](2026-10-06-task-release-0.1.11.md)に版更新と正式公開の読戻しを記録する。
