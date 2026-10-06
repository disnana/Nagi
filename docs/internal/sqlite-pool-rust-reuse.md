# Phase 4: 既存Rust pool/workerの再利用比較

2026-10-06に文書へ反映。2026-10-05の固定sourceによる未採用の読み取り比較。Phase3 acceptance前の調査で、repository/Cargo/依存追加/prototype実行なし。**既存wrapperを使えないとは断定しない。** 下記は固定commitの公開source/docで提供範囲を確認した結果で、候補adapterのborrow/build/障害試験は未実施。

## 確認した版と一次資料

| source snapshot | Cargo manifest上の版・互換候補 |
|---|---|
| [deadpool-rs/deadpool f4efd12](https://github.com/deadpool-rs/deadpool/tree/f4efd12e65b2cc9347f6ca0ee42800cd376ea658) | deadpool-sqlite 0.14.0 / deadpool 0.13.1 / deadpool-sync 0.2.0。sqliteはrusqlite 0.40.0、rust-version 1.95。現在のNagi rustc 1.98.1・rusqlite 0.40.2と整合する候補 |
| [tokio-rusqlite 3aa3388](https://github.com/programatik29/tokio-rusqlite/tree/3aa3388f3ae4050080d175204734f650672deed0) | manifest 0.8.0、rusqlite 0.40.1、Tokio 1、crossbeam-channel 0.5。hooks/bundledのforward featureあり |
| [r2d2-sqlite 097400a](https://github.com/ivanceras/r2d2-sqlite/tree/097400ac6de61b9adf77a34474d3e31cb72e4b39)＋[r2d2 c1b0d9f](https://github.com/sfackler/r2d2/tree/c1b0d9f976e12e97f92554063a7c6bd295eee471) | manifest r2d2_sqlite 0.35.0＋r2d2 0.8.10、rusqlite 0.40。UUID等の追加依存を持つ |

これは各repositoryのmanifest版で、crates.io最新公開版・release済みとは確認していない。全wrapperは現在のNagi依存にない。採用する版、追加crate/transitive dependencies、feature、MSRV/lockfile/link互換性は**依存追加Stopの具体判断**。既存rusqlite版を下げる判断はしない。

## 提供される境界と残るadapter

| 観測項目 | deadpool-sqlite＋deadpool-sync | tokio-rusqlite | r2d2_sqlite＋r2d2 |
|---|---|---|---|
| 取得・所有 | async pool、上限/取得timeout、owned Object。Object保持中はそのpoolから貸さない | clone可能な一connection handle。poolは提供しない | synchronous pool、get_timeout、owned PooledConnection。async待機/実行は別adapter |
| native Txの寿命 | interactの1 closure内でnative Transactionを保持可能な候補。R:Send+'staticなので借用Transactionを外へ返すAPIではない | dedicated threadのcall closure内で保持可能な候補。R:Send+'static、複数call間へborrowed Txを保存するAPIではない | owned checkoutをworkerへmoveし、そのlexical scopeでTxを保持する候補。所有connection＋borrowed Txを外側自己参照structへ保存する解決は提供しない |
| admitted処理のcaller取消 | started blocking closureはcallerのawait破棄で中止されない。Objectをcallerが先にDropするとpoolへ戻り得るため、cleanup完了までadapterがObjectを保持する必要 | call_rawは送信後、workerでclosureを実行し、reply送信失敗を捨てる。receiver取消は送信済み処理の取消にならない | pool自体にasync operation admission/cancellationはない。workerへmoveしたcheckoutの寿命をadapterが保持 |
| cleanup確認前のreuse | mutexによる直列化と取得前recycleはあるが、既定recycleはpoison検査＋SELECT応答だけ。rollback/hook復元成功を証明しない | session/pool/recycleを提供しない。call完了だけではTx cleanup成功とは扱えない | Dropでput_back。既定SqliteConnectionManager.has_brokenはfalse、is_validはSELECT 1。Tx cleanup判定を別に必要 |
| retire | Object::takeでpoolから永久detach、recycle Errでもdiscard可能。poison検査も提供 | worker/connection閉鎖を結果で通知する入口あり。Pool退役方針はない | generic ManageConnection.has_brokenでdiscard可能。既定sqlite managerをそのまま使うと失敗flagを扱わない |
| close | Pool.closeは新get/待機をClosedにしresize(0)。active session/native connection close結果/worker終了のawaitではない。SyncWrapper Dropはbackground destructor | close(self)はnative closeをworkerへ送信し、native Errにはhandleを返す。ただしchannel切断/RecvErrorもOkへ写し、thread JoinHandleを公開しない | 調べたPool APIにclone共通close＋drain完了/worker joinはない。Drop/回収を新close成功と同一視しない |

deadpool根拠: [sqlite lib.rs:71–100](https://github.com/deadpool-rs/deadpool/blob/f4efd12e65b2cc9347f6ca0ee42800cd376ea658/crates/deadpool-sqlite/src/lib.rs#L71)、[sync interact/Drop:123–174](https://github.com/deadpool-rs/deadpool/blob/f4efd12e65b2cc9347f6ca0ee42800cd376ea658/crates/deadpool-sync/src/lib.rs#L123)、[Object take/Drop:50–91](https://github.com/deadpool-rs/deadpool/blob/f4efd12e65b2cc9347f6ca0ee42800cd376ea658/crates/deadpool/src/managed/object.rs#L50)、[pool recycle:161–202 / close:336–349 / return:455–477](https://github.com/deadpool-rs/deadpool/blob/f4efd12e65b2cc9347f6ca0ee42800cd376ea658/crates/deadpool/src/managed/pool.rs#L161)。recycle失敗後には別object作成へ進むため、候補の「retire時Pool取得停止・replacementなし」を既定動作に任せない。

tokio-rusqlite根拠: [call:272–304](https://github.com/programatik29/tokio-rusqlite/blob/3aa3388f3ae4050080d175204734f650672deed0/src/lib.rs#L272)、[close:332–365](https://github.com/programatik29/tokio-rusqlite/blob/3aa3388f3ae4050080d175204734f650672deed0/src/lib.rs#L332)、[unbounded channel/thread/event_loop:374–431](https://github.com/programatik29/tokio-rusqlite/blob/3aa3388f3ae4050080d175204734f650672deed0/src/lib.rs#L374)。bounded inbox、worker panicとreply喪失の区別、verified close/worker終了をwrapperのOkだけで達成したとしない。

r2d2根拠: [sqlite manager connect/is_valid/has_broken:130–173](https://github.com/ivanceras/r2d2-sqlite/blob/097400ac6de61b9adf77a34474d3e31cb72e4b39/src/lib.rs#L130)、[ManageConnection:69–95](https://github.com/sfackler/r2d2/blob/c1b0d9f976e12e97f92554063a7c6bd295eee471/src/lib.rs#L69)、[get_timeout:409–444 / put_back:489–509 / Drop:598–606](https://github.com/sfackler/r2d2/blob/c1b0d9f976e12e97f92554063a7c6bd295eee471/src/lib.rs#L409)。discardはidle補充へ接続する（同source185–242）。memory()はUUID＋共有memory URIを生成するため、提案の`:memory:`複数接続拒否を無断でこのpolicyへ置換しない。

## Nagiが所有する必要のある部分を絞る

三wrapperとも、**一つのclosure/worker scope内でnative Txを保持する形は検討可能**。そのclosureがowned session channelからtyped commandを受ける候補なら、SQL/native借用/commit/rollbackはrusqliteへ委譲し、外側Nagi Txはsenderだけを持てる。closureがcallを終えるまで一sessionを専有する構造・checkoutの保持・cancel後の完了観測は未prototype。単発closure内Txと、複数awaitにまたがる対話型Txを混同しない。

共通adapterに残るのは、Nagi canonical operation/Passing/affine・task捕捉検査、owned Parameters/FromRow/Failure、session command＋epoch、admission/返信喪失、Authorizer管理区間、自動rollback→Aborted、cleanup完了→reuse、失敗→retire/Pool停止、close完了通知。一般SQL parser/trait/effect solverや独自native transaction実装は追加しない。

初めからcustom pool/workerを確定せず、**stock deadpool-sqlite＋小さいsession adapterを比較候補に残す。** 上限・取得待ち・Object/回収・detach・新取得停止は既存poolへ任せられる。strong close等の差が残る場合はgeneric ManagerのTypeへ検証可能なworker handleを置く案と比較する。Manager adapterはlibrary拡張点の利用で、pool algorithmのコピーやforkとは区別する。tokio-rusqliteでdedicated thread/dispatchを再利用する候補も残す。どれが小さく契約を満たすかは未試行。

ただしdeadpool-syncの長いsession closureはTokio blocking poolのthreadを占有する。現在のTokio 1.53.1一次doc（vendor `src/task/blocking.rs:106–136`）はstarted blocking taskのabort不可と、長期workerにはdedicated threadを検討する旨を記載している。SQL単発のinteract benchmarkだけで対話型Txにも適すると結論しない。接続数だけでglobal blocking thread全体の容量を保証しない。

tokio-rusqliteは専用thread/dispatchを再利用できる利点がある一方、強いclose outcome/worker joinは確認sourceのAPIだけで提供済みと扱えない。これを必要とするならadapter/upstream API/狭いclose契約の比較へ戻し、未採用案の保証を黙って縮めない。r2d2は同期pool資産を活用できるが、async取得停止・取消・退役時replacement抑制・close観測に残るadapterが多いため今回は優先度を下げる。いずれも実装不能という判断ではない。

## 判断と未確認範囲

**runtime rusqlite hooksは2026-10-06のQ002で承認済み。追加wrapperのcrate／feature／版はユーザー判断までStop。** 承認後に一connection/一Txでprototype比較し、取消後もadapterがcheckoutを保持すること、cleanup前の再取得禁止、失敗時detach/新get停止、native close結果とworker終了をpositive barrierで確認する。SELECT health check、pool.close/status.size==0、wrapper close Okだけを完了証拠にしない。wrapper固有の自動補充・unbounded queue・blocking thread占有を含め、責任/code量/失敗経路が小さくなる案を選ぶ。現時点でcustom workerを確定案、wrapperを不可能、未実行の試験を成功とは書かない。
