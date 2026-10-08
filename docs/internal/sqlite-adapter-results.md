# Phase 4: 一接続deadpool adapterの比較結果

2026-10-08追記: 以下は当時のprivate試作の記録で、source linkは公開移行前baseへ固定する。現在の公開本体・adapter選択は[公開runtime判断](sqlite-public-runtime-decision.md)を参照。

2026-10-06。Q004で承認されたgeneric deadpool Managerの比較試作。#81はユーザーがmain `ff6f7d4`へマージ済みで、そのcoreと22件のnative試験を維持した。公開Pool／Txは未実装で、Phase 4全体の完了ではない。

検証した実装は[fe741a3](https://github.com/disnana/Nagi/commit/fe741a333e56a1bbc8966b9fd7e540109d2d4b28)、tree `58297cd18df0746046fec2dd7e6c313d352c61b0`。この後に取り込んだmainは同じtreeを保つ。結果文書の追加だけを理由にRust全suiteを繰り返さず、公開後のCIでは最終PR headを検査する。

## 採用した構造と比較の判断

[adapter.rs](https://github.com/disnana/Nagi/blob/676576724829e45b077b58628bfe2417e6cf3673/runtime/src/sqlite_prototype/adapter.rs)は`cfg(test)`の私有モジュール。stock deadpoolのpermit・queue・checkout・recycleを使い、SQLiteの解析・bind・Transactionは既存rusqliteへ任せる。独自pool algorithm、SQL parser、unsafe、自己参照型は追加していない。

native workerがowned Objectをsession開始要求と一緒に受け取り、[共通session](https://github.com/disnana/Nagi/blob/676576724829e45b077b58628bfe2417e6cf3673/runtime/src/sqlite_prototype/session.rs)のcleanup完了まで保持する。Tokio上の別cleanup taskへ所有者を移さない。WorkerHandle Dropはroot senderを閉じ、startup guard・ledger・reaperがnative closeと実JoinHandleを観測する。ledgerはTx senderを延命せず、ObserverもPoolを延命しない。

新しいlockfile項目はdeadpool 0.13.1とdeadpool-runtime 0.3.1の2個だけ。選択featureはmanaged／rt_tokio_1、default featuresなし。既存Tokio・rusqlite・SQLiteの解決版は変更していない。[依存](../../runtime/Cargo.toml)と[判断](sqlite-pool-adapter-decision.md)を参照。

一接続では、この分担でcleanupとcloseの承認契約を検査できた。generic Managerを次の縦切りの候補として維持する。ただしadapterは586行、専用workerに加えてpool reaperを1thread持つ。native close／joinを扱う責任はdeadpoolだけでは解消しない。多接続の終了条件と公開Optionsへの接続が成立するまでは、最終的な公開Poolの採用判断を終えたとは扱わない。

## 見つかった問題と共通原因

| 分類 | 実際の反例 | 原因と修正 |
|---|---|---|
| 私有試作P1・修正済み | thread join完了を先に公開し、native closeの失敗がledgerに載る前にcloseが完了判定した | 完了と結果公開を混同。terminal causeを先に公開し、同lockでcounter更新・live記録除去を確定してから通知 |
| 私有試作P1・修正済み | create取消後、旧workerのjoin前に別workerが起動。max_size=1でもpending workerが2 | stock poolの論理slot解放とnative資源の終了を混同。単一接続のManager.createはlive記録が空になるまで待ち、新登録とclosing判定を同lockで行う |
| 私有試作P1・修正済み | Object::takeがpermitを先に返し、Manager.detach／WorkerHandle Drop前に別workerを作れた | stopping flagやDrop順への依存。同じlive-empty条件で処理し、経路ごとの先回りflagを増やさない |
| 私有試作P2・修正済み | 完了したStateを履歴Vecに残し、closeが全履歴を走査する構造 | live記録と累積観測を混同。join後にStateを除き、created／native close／joinedのcounterと最初のfailureを保持 |

これは比較中の新しい私有実装の反例であり、公開済みの旧Dbで同じ欠陥を再現したという報告ではない。有限の試験・レビューでは新たなP0は確認していない。P0が存在しないという保証ではない。

tests-only段階の未定義adapter（E0432）、detach seam未定義（E0560）と、上表の実際の契約違反を分けた。途中実装の非Send MutexGuardによるcompile失敗も成功扱いしていない。RED sourceとログは[検証artifactの説明](../../benchmarks/results/sqlite-adapter-2026-10-06/README.md)に対応を残した。既存native22件のassertは変更していない。

## 継続検査する契約

[adapter試験](https://github.com/disnana/Nagi/blob/676576724829e45b077b58628bfe2417e6cf3673/runtime/src/sqlite_prototype/adapter_tests.rs)20件の主分類。1件を複数分類へ加算していない。

| 契約 | 件数 | 代表する観測 |
|---|---:|---|
| acquire／close後のadmission拒否 | 2 | create中・取得直後にcloseしてもuser BEGINを開始しない |
| cleanup前のcheckout保持 | 2 | caller取消・rollback pendingでも次のcheckoutへ返さない |
| create取消／runtime寿命 | 2 | 起動取消とTokio runtime Drop後もnative終了責任が残る |
| recycle／replacement停止 | 2 | recycle Err・retire後に健康な代替workerを暗黙生成しない |
| close／native close失敗 | 2 | timeout／取消後もclosingを維持し、native close Errをjoin後も保持 |
| Drop／detachを迂回する破棄 | 4 | resize(0)、WeakPool失効、最後のhandle、active Txが残る経路 |
| failure公開／retire | 3 | begin返信取消・worker panic・join通知とcauseの順序 |
| native worker上限／live記録解放 | 3 | create取消とstock detachの隙間でも単一worker、完了記録を解放 |

元の[native22件](https://github.com/disnana/Nagi/blob/676576724829e45b077b58628bfe2417e6cf3673/runtime/src/sqlite_prototype/tests.rs)は、SQL／authorizer／bind／decode／ACTIVE・ABORTEDを9件、cleanup／reuse／retire／outcomeを7件、取消／inbox／返信喪失を4件、native closeとjoinを2件で検査する。ordinary SQL Errだけで先行効果が戻ったとは扱わない。[先行基盤の結果](sqlite-session-results.md)を参照。

## ローカルの結果

Linux x86_64、既存warm target、locked／offline Cargo、debug profile。socketを使う既存試験には実行環境のnetwork権限を使った。

| 検査 | 結果と保証範囲 |
|---|---|
| `cargo test --locked -p nagi-runtime sqlite_prototype` | 43件成功。native22＋adapter20＋比較1、失敗・ignoreなし |
| `cargo test --locked` | 92 suite・874成功、失敗・ignoreなし。上記を含む |
| runtime単独 | 172 unit＋5 doctest成功。上記43件を含む |
| workspace all-target clippy／fmt | 成功。clippyは`-D warnings` |
| fuzz smoke | seed 305419896、1000 text mutation、95 checked Low／emit、16 bounded native、panic 0。coverage-guidedではなく、新SQLite APIの意味論探索でもない |
| [予定High／手書きLow](../../compiler/tests/sqlite_contract_inputs.rs) | 2 testで23組46sourceの構文・元行anchorを検査。semantic結果は未配線のまま |
| 独立source review | 別のSolが登録・終了結果公開・live記録除去・stock detachの順序を確認。追加P0/P1指摘なし |

件数は包含関係にあり、全suiteに43件や172件を加算しない。parser成功を新Txのcompile-pass／compile-fail成功と数えない。新APIのHigh→Low→Rust build／runはまだ確認できない。

## 同条件の概測

[比較入口](https://github.com/disnana/Nagi/blob/676576724829e45b077b58628bfe2417e6cf3673/runtime/src/sqlite_prototype/comparison.rs)で、同じnative core・hooks・SQLを使い、begin→`SELECT 1 AS n`→rollbackを測定した。各8回warmup後、32 batch×128 Txを両backendで実行し、batchごとに実行順を交互にした。各4096 sample、計8192 sampleを[CSV](../../benchmarks/results/sqlite-adapter-2026-10-06/samples.csv)へ保存した。両方でSQL結果・native close・joinもassertする。

| debug・warm・直列・一接続 | p50 | p95 |
|---|---:|---:|
| private direct Driver | 106.370 µs | 141.402 µs |
| generic deadpool Adapter | 114.402 µs | 152.410 µs |

この観測ではadapterのp50が約7.6%大きい。VM上の単一条件で、信頼区間・因果的overhead上限・本番性能の比較ではない。並列契約試験中の値と混ぜず、最後の単独測定を使った。release、競合、多接続、HTTP throughputは未測定。

両fixtureとも2thread（directはworker＋join observer、adapterはworker＋pool reaper）。adapterには1 Txごとの`Box<CheckoutOwner>`追加箇所があるが、allocator countは測定していない。常時memory、Future frame、binary size、compile time、以前のNagi Future +32 byteの原因も未測定。速くなった、costが不変、zero-copyであるとは報告しない。

再実行方法と測定条件は[README](../../benchmarks/results/sqlite-adapter-2026-10-06/README.md)、数値とsource treeは[summary.json](../../benchmarks/results/sqlite-adapter-2026-10-06/summary.json)。

## 未完了と次の順序

多接続の終了観測、取得期限、巨大capacityのsourceレビューと先行oracleは[次の縦切りメモ](sqlite-public-slice-plan.md)へ分けた。これらの反例候補を実行済みの不具合として加算しない。

1. 公開runtimeで多接続とOptionsの取得期限を接続する。一接続のlive-empty条件を多接続へ流用しない。健全active workerを妨げず、取消後のnative上限・cleanup・`acquire_ms=0`／有限待ちを同時に検査する。
2. runtime公開入口とcanonical registry／capture facts／sealed SQLを一緒に接続する。Txのnested payload、function値、async capture、nonDebug field、generic copy、SQL所有化の予定ケースを本当のpositive／negativeへ昇格する。
3. 元位置付きHigh／保存Low／手書きLow、旧Db、実DB、4 OS、生成探索、縦切りの性能を確認してPhase 4 acceptanceを判断する。

現在確認できるのは私有・一接続adapterの上記契約である。Nagi利用者向けPool／Tx API、capability拒否、public FailureのDebug、SQL opt-in、公開取得期限はまだ保証できない。G-TX／G-POOLは予定のまま。Phase 5のAuth Scopeも開始していない。

## CI

既存Linux全suiteと4 OS package matrixの`sqlite_prototype` stepが43件を実行する。予定High／Lowの専用stepは46入力をparserで検査する。stepは#81で登録済みで、今回の差分でCI条件を弱めていない。

head `a608f1a5585eae8b27e32eced0299070269f3b57`の[checks run 37402576311](https://github.com/disnana/Nagi/actions/runs/37402576311)と[website run 37402576023](https://github.com/disnana/Nagi/actions/runs/37402576023)はattempt 1で成功した。4 OSの各実ログでnative22＋adapter20＋比較1の43件、parser2件の成功を確認した。[job別の記録](../../benchmarks/results/sqlite-adapter-2026-10-06/ci-source-proof.json)に件数とIDを保存した。Linux全検査、IntelliJ IDEA、PyCharm、merge gateも成功。VSIX packageは変更対象外でskip、releaseもskipである。

この後はartifactの改行・記録hashと次の設計メモだけを更新し、compiler／runtime／Cargo／CIは同じbytesを保つ。最終headのCIはPR #82で再確認する。#81の成功を新adapter成功に流用せず、各headと検査範囲を分けた。mainへのマージ、版更新、releaseは行っていない。
