# SQLite Pool adapter: 採用した比較方針

2026-10-08追記: Q004のdeadpoolは比較試作として採用した履歴。公開実装はユーザー承認下で既存Tokio Semaphoreとlazy専用adapterを選び、deadpool/deadpool-runtimeを削除する。理由・互換性・終了責任・検証は[公開runtime判断](sqlite-public-runtime-decision.md)を参照。以下の比較とcapability表は履歴として保持する。

2026-10-06の判断資料。Q004で下記の依存とcapability表を承認済み。[PR #82](https://github.com/disnana/Nagi/pull/82)ではprivate一接続adapterのbuild／実行・4 OS CIを確認し、main `7999bab`へ反映された。[比較結果](sqlite-adapter-results.md)に43件・全suite・測定と未完了範囲がある。以下の候補比較は承認前の調査記録として残す。公開Pool／Txや多接続まで実装済みとは扱わない。[承認記録](open-questions.md#q-004-sqlite-poolのwrapper依存と未指定capability)と[ADR 010](adr/010-sqlite-transaction-boundary.md)を参照。

## 推奨: generic deadpoolのManagerを使う

`deadpool 0.13.1`のmanaged poolに、native workerのowned handleを持つManager adapterを接続する。poolの上限・待機・checkout・回収はdeadpool、SQLiteのSQL解析・native Transactionは既存rusqliteへ任せる。Nagi側に残すのはsession、cleanup結果、closing／failedの共有状態と完了観測である。pool algorithmやnative transactionをコピーしない。

承認した比較試作の依存は以下一行で、mainへ反映済み。これ以外の追加・更新が必要なら、理由と差分を示して判断へ戻す。

```toml
deadpool = { version = "=0.13.1", default-features = false, features = ["managed", "rt_tokio_1"] }
```

公開されたmanifest／dependency metadataを2026-10-06に確認した。deadpool 0.13.1は2026-08-26公開、yankなし、MSRV 1.85、MIT OR Apache-2.0。選択featureで新しく必要になるcrateはdeadpoolとdeadpool-runtime 0.3.1。Tokioは現在の依存を使い、rusqlite 0.40.2、bundled、libsqlite3-sysを変更する案ではない。async-std／smol／serde／unmanagedは有効化しない。lockfileで実際の解決結果とfeature合成を確認し、予想外の追加・更新が必要ならその差を判断へ戻す。

一次資料: [deadpool 0.13.1 metadata](https://crates.io/api/v1/crates/deadpool/0.13.1)、[dependencies](https://crates.io/api/v1/crates/deadpool/0.13.1/dependencies)、[deadpool-runtime 0.3.1](https://crates.io/api/v1/crates/deadpool-runtime/0.3.1)、[dependencies](https://crates.io/api/v1/crates/deadpool-runtime/0.3.1/dependencies)、[Manager](https://docs.rs/deadpool/0.13.1/deadpool/managed/trait.Manager.html)、[Pool](https://docs.rs/deadpool/0.13.1/deadpool/managed/struct.Pool.html)、[Object](https://docs.rs/deadpool/0.13.1/deadpool/managed/struct.Object.html)。承認前の調査ではrelease archiveのSHA-256をcrates.io metadataに照合して読み取り、Cargoへの追加・第三者コードの実行は行わなかった。承認後の比較試作とCIは冒頭の#82を参照。

| 部分 | ownerと候補構造 | 必要なoracle |
|---|---|---|
| acquire | deadpoolのowned Objectと待機上限を使う。closing ledgerを取得前後で確認し、予約済みでも未admittedならuser sessionを開始しない | closeと待機acquire／取得直後のraceをpositive barrierで検査 |
| session | adapterがObjectをcleanup完了まで保持し、worker lexical scopeのnative Txへtyped commandを渡す | caller取消後も他checkoutへ返らず、cleanup後にだけ再取得 |
| recycle | 前sessionの終了結果とworker健全性を確認する。SELECT health checkだけで代替しない | rollback失敗・worker panic・unknown状態をIdleへ戻さない |
| retire | Object::take／Manager::detachと共有failed状態を接続する。Poolを閉じ、Manager::createもfailed後のreplacementを拒否する | recycle Errでdeadpoolが補充へ進む経路を含め、新begin不成立 |
| close | deadpool.closeは新get停止の一部。adapterがnative close結果と全worker joinを観測する | idle／active／取消／timeout／native close失敗、clone共通closing維持 |

closing／failedとadmissionの判定は単一の小さいledgerで直列化する。私有prototypeのDriverはsender cloneとcloseの全raceを扱わないため、そのままpublic Poolに流用しない。workerの終了通知とjoinを区別する。prototypeの一worker一join観測threadは試験用であり、本実装のthread構成を決めた証拠ではない。close取消時のjoin責任と最後のhandle Dropを含め、adapter試作で所有者を固定する。

独立レビューで、deadpoolの`resize(0)`／Pool Drop／WeakPool失効時のObject破棄はManager::detachを経由しないことを確認した。detach callbackだけを全cleanupのownerにはしない。WorkerHandle Dropは閉鎖要求へ接続し、ledger側の終了所有者はstarting／idle／active／detachedの全workerとnative close／joinの完了を保持する。senderの強参照を持ってsession EOFを妨げない。get取消でManager::create Futureが破棄される場合は、worker起動前に登録したstartup guardが責任を引き継ぐ。

closeはin-flight createも待つ。Manager::createが始まる前にclosing判定とstarting登録を同じcritical sectionで行い、worker完了・引渡し・取消で未完了件数を確定させる。close後に新workerが登録される経路を拒否し、starting件数だけを減らしてjoin責任を消さない。deadpool getが閉鎖後にObjectを返すraceでもuser BEGINを開始せず、そのowned handleを同じ終了所有者へ返す。これは終了責任のadapterであり、別のslot待機／pool公平性algorithmを追加する案ではない。late create、idle discard、active返却、Object::take、最後のPool Dropを別barrierで検査する。

### 論理slotとnative workerの終了を分ける

比較初版では、Manager.createの取消後にdeadpoolのpermitが返り、旧workerのjoin前に次のworkerを起動できた。max_size=1でcreated=2となるbarrier反例を確認した。Object::take後のWorkerHandle Dropにも同じ論理slotとnative終了の差がある。stock poolのsizeだけをnative worker上限の根拠にしない。

Object::takeのstock実装はManager.detach／WorkerHandle Dropより先にpermitを返すため、stopping flagだけでは同時createの隙間が残った。今回のmax_size=1比較は、Manager.create内でlive recordが空になるまでnative close／joinを待つ単純なfenceを採る。正常なObject再貸出はrecycle経由、active Object中はstock permitを取得できないため、create入口で残るlive recordは先に論理slotを返した旧workerである。新登録とclosing判定は同じledger lockで確定する。stock permit・queue・公平性を置き換えず、取消だけをPool failedにするpolicyも足さない。

これは単一接続の比較限定で、multi-connectionへ全live worker待機を流用しない。公開APIへ進む際は、健全active workerを妨げずnative上限とcleanupを両立する条件を別に検証する。公開Optionsの接続時にはfenceもacquire_ms=0／有限待ちの条件に含め、途中から無期限待ちへ変えない。

完了workerのStateを全履歴として保持する初版のledgerも、公開runtimeへ流用しない。terminal causeを公開し、同じcritical sectionで完了件数へ集約してlive recordを除く。累積created／native close／joinedの観測は保ち、closeの完了条件を履歴Vecの全走査に依存させない。反復取消とtake/drop/createで、未終了record数と累積件数を別々に確認する。

この節の反例を元にした修正版は、一接続のbarrier試験と全suiteで確認した。multi-connection／公開APIの完成は示さない。

## ほかの候補を今すぐ採らない理由

- **deadpool-sqlite 0.14.0:** owned checkoutと短いinteractには適する候補。deadpool-syncの公開APIからConnectionを所有値として取り出す経路を確認できず、Object::takeはSyncWrapperを返す。Dropはbackground destructorなので、native close結果とworker joinまで完了した根拠にはならない。長い対話sessionはTokio blocking threadを占有する。generic Managerとの比較前に「利用不能」とは結論しない。
- **tokio-rusqlite 0.8.0:** 専用worker／dispatchを再利用できるが、確認したcloseはchannel切断をOkへ写し、thread JoinHandleを公開しない。unbounded queueも初版Optionsのinbox契約と別である。forkや保証の縮小を先に行わない。
- **r2d2:** 同期poolとして成熟しているが、async acquire／clone共通close／取消後の完了観測に残るadapterが多い。
- **独自pool:** 追加依存を減らせる一方で、上限・待機・回収・公平性をNagiが実装し続ける。現在の規模では第一候補にしない。

generic deadpoolでもnative closeやsessionを自動で保証してくれるわけではない。比較試作でadapterが膨らむ、safe借用で成立しない、承認されたpolicyを満たせない場合は不採用を含め再判断する。source読取だけで候補が成立したと報告しない。

## registry配線前に固定するcapability

下表は未指定だったDebug／shared等も含む**承認済みの初版値**。公開checkerの配線は未完了。全resourceのSerdeと新Actor Charge対応はなし。署名／markerと実payloadを区別し、Txのtask転送・永続格納禁止をnative inline stateにも適用する。ユーザーの同名classはこの制限の対象ではない。

| resource | Copy | equality | field保存 | shared | Debug |
|---|---|---|---|---|---|
| Pool | 不可 | 不可 | 可 | 可 | 状態のみ |
| Tx | 不可 | 不可 | 不可 | 不可 | 不可 |
| Parameters | 不可 | 不可 | 可 | 不可 | 不可 |
| Options | 不可 | 不可 | 可 | 不可 | 設定値 |
| BeginMode | 可 | 可 | 可 | 可 | variant |
| Failure | 不可 | 不可 | 可 | 可 | kind／outcome／retiredのみ |
| FailureKind／Outcome | 可 | 可 | 可 | 可 | variant |

TxはnonClone、ParametersもnonClone。Poolのcloneはclone_poolのみ。local Option／Result、owned関数委譲、同task awaitはTxのfield保存やtask転送と同一扱いにしない。Failureのcause複製は明示copy operationだけで、DebugにSQL／bind値／cause本文を足さない。公開messageとcauseに含まれるDB診断が機密を含まないという保証は追加しない。

## 実装と検証の順序

1. 選択crate／feature／lockfileの差を確認し、一接続のManager adapterへ同じnative contract oracleを接続する。
2. cleanup前checkout保持、close/admissionのrace、retire後replacement停止、native close／joinと最後のDropを検査する。
3. 成立した範囲からruntime公開入口とregistry／capture facts／sealed生成を同時に接続する。未完成runtimeへcheckerだけを先行公開しない。
4. High／保存Low／手書きLowのpositive／negative、元位置、SQL opt-in、旧Db、実Cargo／4 OS、fuzz／生成探索と比較測定を行う。

Phase 4の完了条件はこの文書の採用ではなく、段階計画のacceptanceである。merge・版更新・releaseの承認は含まない。
