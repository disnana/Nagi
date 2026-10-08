# ADR 010: SQLite PoolとTransactionの境界

状態: 2026-10-06、ユーザーがQ002の選択1を承認。公開API・SQL制限・終了契約とruntime rusqlite `hooks`を採用する。実装・検証の完了とは区別する。追加wrapperのcrate・版・featureはQ004で別に判断した。同日にgeneric deadpool =0.13.1（managed／rt_tokio_1、default featuresなし）、deadpool-runtime 0.3.1の比較試作と[capability初版表](../sqlite-pool-adapter-decision.md#registry配線前に固定するcapability)をユーザーが承認した。既存Tokio／rusqliteの版は維持する。

## 2026-10-08の実装判断

正式化前に長期的な妥当性を優先するユーザー承認に基づき、deadpool試作から既存Tokio Semaphoreとlazy専用adapterへ移行する。限定vendorは未採用で、新依存・版更新を加えずdeadpool/deadpool-runtimeを削除する。独自waiter公平性や取消schedulerは作らずTokioへ任せ、idle checkout/returnと既存native ledger/observerの接続をNagiが保守する。取得ごとの予算はtask-localではなく明示引数で渡す。

Optionsは全slot Layoutを検査する構造から、usize/正数/Semaphoreとnative時間/busy_ms i32を検査する構造へ変わる。実container増分の予約失敗にはALLOCATIONを追加し、Drop返却失敗も停止・退役・cause公開・actual joinまで追跡する。openはlazy native起動を保つ。SQL/affine Tx/0ms/同じ取得予算/登録で期限終了/終了責任/旧Db契約は維持する。判断比較、構造化cause、公開API、配布証拠は[公開runtime判断](../sqlite-public-runtime-decision.md)を参照する。

以下のdeadpool/Object::takeとprivate試作の記述は決定履歴であり、現在の公開pool依存を示すものではない。

## 前提と分担

Phase 3の#80はhead `35038940`で4 OS・editor・site・merge gateが成功し、main `f10cb64`へ反映済み。両者のtree `e45dbede`は一致する。旧Db/API、High／Lowの意味論、Rust backendを保つ。

採用する署名・資源・capability・値とSQL受理範囲は[API契約](../sqlite-pool-proposal.md)の表に固定する。`std.db.sqlite`、owned Parameters、明示Options／BeginMode、Failureのprimary／cleanup causeとoutcomeを採る。容量・期限の数値defaultは追加せず、0msは即時に条件が成立すれば成功する。

SQLiteの解析・bind・row metadata・native Transactionはrusqlite 0.40.2へ任せる。Nagiは型付き操作、所有権、task転送、受理済み処理とcleanup／closeの完了を扱う。汎用SQL parser、trait/effect solver、独自native transaction、自己参照型、unsafeは追加しない。

pool／dispatchの実装は[既存Rust比較](../sqlite-pool-rust-reuse.md)を基に選ぶ。最初の一接続prototypeで専用threadやbounded channelを使うことは、独自pool algorithmの採用やwrapper不採用を意味しない。Q004の範囲でManager adapterを試作する。予想外の追加依存・版更新が必要なら、transitive dependenciesと互換性を示して確認する。

## 所有とSQL

TxはnonCopy／nonClone／nonshared／nonSerde。field・enum等の永続格納とtask転送を禁止し、同task内のawait、owned委譲、local Option／Resultを許す。commit／rollbackはFuture作成時からTxをconsumeする。async関数値の実引数を捕捉として検査し、戻りFuture型だけで転送可能と判断しない。関数pointerの署名は捕捉payloadではない。既存Future保存・返却制限と旧resourceの受理は維持する。

新Txのuser SQLは一文・匿名`?`。query／allはSQLiteがreadonlyかつ返却列ありと判断する文、execは返却列なし。行型は既存の標準FromRowが生成できるclass fieldに限る。NULL・値型・範囲・bind数は実行時にも検査する。opt-in schema検査の成功を実DB一致やTx安全の証明としない。動的Parametersのbind未検査を明示する。

常設safe Authorizerでuser SQLのTransaction全variant・Savepoint・Pragma・Attach・Detach・Unknownを拒否する。virtual table／extension／raw Connectionは公開しない。管理BEGIN／COMMIT／ROLLBACKはprivate区間のみ。user statementのprepare・step・reprepare・finalizeまでhookを保ち、管理権限でFromRow／user callbackを実行しない。keywordやSQL文字列の自作解析を根拠にしない。

## cleanupと終了

一Txは一接続を専有する。取消は受理済みSQLを戻さない。未poll終端、送信前取消、begin返信喪失、最後のTx Dropでもcleanup責任をworkerへ残す。満杯queueへのDrop.try_send成功だけに依存しない。

native rollbackの結果、autocommit、statement破棄、user hook状態、worker健全性を確認する前に再利用しない。native TransactionのDropはrollback成功の証拠ではない。普通のSQL／bind／decode Errでnative Txがactiveなら継続でき、自動rollbackはAbortedとして後続SQLを拒否する。

`Err`はstatementの変更がゼロであることを保証しない。SQLiteの`INSERT OR FAIL`やAFTER trigger途中の失敗では先行する変更がTx内に残り得る。禁止PRAGMAの拒否と、許可された親DMLの巻き戻しは別の観測である。変更を破棄する場合は明示rollback、または未終端sessionのEOF cleanupを確認する。各statementの暗黙savepointや全Errでの自動abortは採用していない。

cleanup失敗・worker panic・状態不明では接続を退役し、Poolの新取得を止める。既存active Txは終端を続け、自動replacement／retryは追加しない。COMMIT完了とcleanup失敗は両方保持する。返信喪失の結果はUNKNOWNで、再実行可能と決めつけない。

close開始後はclone共通で新取得を止め、active Txのcleanup、native close結果、worker終了を待つ。close timeout／取消でclosingを解除しない。再closeで完了待ちを許す。成功通知をworkerが送っただけではthread joinの証拠にならない。最後のPool Dropは閉鎖要求であり、非同期cleanup完了のAPIではない。

## Pool adapterの終了記録

stock deadpoolの論理slot返却と、native workerのclose／join完了は同じ出来事ではない。startup取消やObject::take後の退役workerが残る場合は、Manager.create内のcleanup fenceで終了を確認してからreplacementを起動する。独自のpool待機／公平性algorithmや、取消をfailedへ変える公開policyは足さない。一接続のprivate比較では、Object::takeがdetach／Dropより先にpermitを返すraceを避けるため、create入口のlive recordが空になるまで待つ。max_size=1の正常再貸出はrecycle経由で、健全active workerを待つ一般gateではない。multiへこの全live待機を流用せず、健全active workerの並列性とnative上限は別に検証する。公開acquire期限を接続する際はfence待ちも期限に含める。

terminal failure／causeをledgerへ公開してから完了joinを通知する。通知を先に出すと、native close失敗を取り落とすraceが生じる。完了recordは同一ledgerのcounterへ集約し、未完了workerだけを保持する。これらはQ002／Q004の終了責任を実現する内部方針で、private一接続の反例から検査する。公開Pool保証の実装・検証完了とは区別する。

## 多接続へ向けたprivate縦切り

#82の一接続比較をmainへ反映した後、[native容量と独立join](../sqlite-multiconnection-design.md)を先行回帰で検証する。workerごとのobserverがnative JoinHandleを所有し、健康な別workerの寿命へ終了観測を依存させない。起動不成立をfake joinと数えず、公開取得期限とcapabilityは別の未完了範囲へ残す。private試作の内部設計であり、公開Pool契約の実装完了やLow/言語意味論の変更ではない。

## #84後のprivate取得予算

#84はmain `e7aff1d`へ反映済み。独立observerとnative容量を保持し、次は[取得予算の設計](../sqlite-acquire-budget-design.md)に従ってlogical slot待ちからnative登録までを同予算で検査する。登録後のready／BEGIN等は取得期限へ含めない。予算scopeと失敗分類の先行回帰から始め、公開Options／Poolの実装完了とは区別する。

## 最初の検証と段階

1. このADRとinvariantsを記録する。
2. public Poolを先に登録せず、private一接続sessionの失敗テストを作る。未実装によるcompile failureと実SQLによるcontract failureを区別する。
3. lexical native Tx、常設hook、typed command、EOF cleanupをsafe Rustで試作する。正常・Err・panic・取消・満杯inbox・未poll終端をpositive barrierで検査する。
4. transaction-control、pragma TVF、trigger／view、reprepare、bind、操作形、自動rollback、native closeとjoinを実SQLiteで検証する。
5. 同じ契約で既存wrapperのadapterと比較して内部実装を選び、Nagi registry／捕捉facts／sealed生成／SQL collectorを縦切りで接続する。
6. pass／fail、元診断位置、High／保存Low／手書きLow、Rust build／run、旧Db、runtime独立build、4 OS CI、fuzz／生成探索と比較測定を行う。

private prototype成功を公開API完成とは報告しない。新syntax、Future保存解禁、未承認の追加依存、safe hookで満たせないSQL反例、一般effect／region解析やunsafeが必要な場合は、保証を下げず根拠と代替案を示して止める。
