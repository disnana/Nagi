# Phase 4 Pool／affine Tx: 実装前の判断案

2026-10-05。未採用の候補。#80 test-only head `ca9362e5`の4 OS CI待ち中に、既存`sqlite-pool-proposal.md`を具体化した読み取り設計。repository変更・Cargo・prototype実行・追加agentなし。Phase 3 acceptance後に判断する資料であり、公開API・policy・依存featureを採用したとは扱わない。

## 推奨する一案と承認が必要な範囲

新SQLite Pool専用workerで、worker-localなrusqlite Transactionと外側のowned nonClone sessionを分離する。新TxのSQLは一文ずつ実SQLiteでprepareし、safe Authorizerで利用者SQLのtransaction-control／savepoint／connection設定変更を拒否する。明示consumeだけが終端を要求し、Dropはworkerへcleanup責任を残す。cleanup失敗・worker喪失では接続をretireし、Poolを新規取得停止へ移す。自動replacement・retry・interruptは初版に入れない。

**この案を実装する前には、少なくともAPI名・新エラー表現・各policyの明示指定・SQL受理範囲・runtimeの`hooks`追加について判断が必要。** 既存Dbの値やrusqlite defaultを新APIへ暗黙流用しない。依存feature追加は、依存追加なしでも既存Stop条件の対象として明示する。

## 依存とsafe APIの実在

根拠のvendor sourceは`/workspace/toolchains/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`以下。以下のrusqlite根拠は`rusqlite-0.40.2/`、SQLite header根拠は`libsqlite3-sys-0.38.2/sqlite3/sqlite3.h`の行。

| 確認した事実 | 根拠・意味 |
|---|---|
| lockfileのrusqliteは0.40.2 | repository `Cargo.lock:582–593`。version変更は不要な候補 |
| runtimeは`features=["bundled"]` | `runtime/Cargo.toml:12`。compilerだけがoptional rusqliteに`bundled/hooks/limits`を指定（`compiler/Cargo.toml:7–16`） |
| hooksはfeatureでgateされる | rusqlite `src/lib.rs:134–135`。`Cargo.toml:114`の`hooks=[]`は新crate依存を増やさないが、有効化は必要。workspaceのfeature合成だけでruntime単独／生成appでも使えると結論しない。生成appのmanifestはruntimeをpath依存で入れる（`compiler/src/emit.rs:1602–1606`） |
| Authorizer登録はsafe Rust API | `src/hooks/mod.rs:442–451`の`Connection::authorizer`。callbackは`FnMut(AuthContext)->Authorization + Send + 'static`。Nagi側にunsafe／FFI wrapperを追加する必要はない候補。crate内部の既存unsafeと、新しいNagiのunsafe追加は区別する |
| SQLite actionを構造化して受け取れる | `hooks/mod.rs:68–189`にTransaction、Savepoint、Pragma、Attach、Detach、Function、Unknown等。`Authorization::Deny`は拒否、Ignoreは意味を変えて継続するのでTx拒否には使わない（`:323–339`） |
| 一文検査は既存safe prepareで可能 | `lib.rs:792–797`はSQLiteのtailを再prepareし、次の実statementがあればMultipleStatementを返す。コメント／引用内のsemicolonを自前splitしない。新parser・`extra_check`追加・Batch iterator用新直接依存は、この候補には不要 |
| native Txはborrowed、終端はconsume | `transaction.rs:63–65,107–129,187–205`。ConnectionとTransactionの自己参照structを作らず、workerのlexical scopeへ置く |
| native Dropはcleanup成功の証明ではない | `transaction.rs:220–246`はautocommitを確認し、DropはfinishのResultを捨てる。close／cleanupのResultを別に観測する |

**特に注意:** 0.40.2の`TransactionOperation`はBegin／Release／Rollback／Unknownで、文字列`COMMIT`はUnknownへ写る（`hooks/mod.rs:305–319`）。利用者SQLは`AuthAction::Transaction { .. }`をvariant全体で拒否する。COMMITの想像上のenum variantやRelease扱いを根拠に許可しない。native終端だけを許すprivate区間は、workerのhardcoded終端operationから入る必要がある。

## 公開APIの最小候補

module名`std.db.sqlite`と以下の名称は**判断用の仮名**。SQLiteとPostgreSQLを同じ資源へ統合しない（`docs/library-design.md:60–74`）。既存Db／db_*／FromRow／Sqlを置換しない。

| 新しい入口の候補 | 最小の意味・未決事項 |
|---|---|
| `open(path, options) -> Future[Result[Pool, Failure]]` | 専用connection workersを所有。Pool共通のclosing状態を保持。optionsは必須で、引数省略版なし。pool共有／cloneの公開方法は`shared[Pool]`利用か明示clone operationかを判断する |
| `clone_pool(view[Pool]) -> Pool` | 明示clone operationの候補。推奨する最小の公開clone方法とし、同じclosing ledgerを参照する。PoolをNagi Copyにはしない。`shared[Pool]`の公開利用範囲は別に判断する |
| `begin(view[Pool], mode) -> Future[Result[Tx, Failure]]` | idle slotの取得とBEGINを一つのowned session取得にまとめる。外側Lease APIは初版に公開しない。mode必須。begin応答喪失でもworkerがlease／cleanupを所有 |
| `exec(view[Tx], sql, parameters) -> Future[Result[i64, Failure]]` | 一文・owned Parametersをconsumeする候補。複数文の従来Db.execと異なる新API。rowsを返すstatementの扱いも明記する |
| `query[T]／all[T](view[Tx], sql, parameters)` | 既存class行変換を再利用する候補。固定id/name/age bindは新APIへコピーしない。Parameters境界は末尾の比較Bを推奨するが未採用。SQL型、Passing、返却形・RETURNINGの扱いを実装前に固定する |
| `commit(tx)／rollback(tx) -> Future[Result[unit, Failure]]` | Txをmoveでconsume。unpolled／send前取消もhandle Dropからcleanupへ進む。成功はnative終端＋所要cleanupを確認した結果。送信後のreply喪失は結果不明として区別 |
| `close(view[Pool], deadline) -> Future[Result[unit, Failure]]` | 全cloneに取得停止が見える。成功はactive session終了、cleanup、connection close、worker終了を観測。timeoutはclose完了を意味せず、closingを維持する。再度closeで待機できる候補 |

TxはnonCopy／nonshared／nonSerde／field保存不可／task transfer不可。同一taskの通常awaitとownedな関数委譲は許す。戻り値がFuture[unit]でも内部に捕捉されたTxを落とさないprivate transfer factsが必要。既存Future local／returnの拒否を解禁しない（詳細は`sqlite-pool-proposal.md`）。

エラーの推奨は新SQLite専用`Failure`と判別可能なkind／outcome／primary cause／cleanup cause。既存ErrorのfieldやErrorKindを増やして既存Rust match／struct literalを壊さない（`runtime/src/lib.rs:28–40,68–75`）。新Failureの正確な公開field／accessor／native formatterは未決で、API表の承認対象にする。message prefixのparseだけでcommit outcomeを判断させる案は避ける。

初稿の固定id/name/age署名は比較Aとして取り下げ、新Pool/Txの推奨APIには含めない。旧Dbの署名を保持することと、新APIのbind境界を選ぶことは別。SQLは既存`str`から内部Sql::Static／Ownedへ渡す候補で、publicなSql型は要求しない。Tx引数はReference、終端TxとParametersはMoveの候補。正確なNagi/Rust借用・Future捕捉が受理されるかは未検証。

初版のoptionsは`options(connections, queue_capacity, acquire_ms, busy_ms)`で全値を必須とする形を推奨する。0／無期限／上限の表現と許容値は判断待ち。begin modeとclose deadlineは各呼出しで明示する。

## 必須の明示policy: 値はまだ選ばない

| 項目 | 決める必要がある内容 |
|---|---|
| capacity | connection数、0／負値／上限超過の拒否。`:memory:`はconnectionごとに別DBなので、初版で多接続を拒否するか別公開仕様にするか。勝手にshared-cache URIへ変換しない |
| queue | admission queue容量と満杯時の待機範囲。古いDbの64を流用しない。worker／session内部値を公開しない場合でも、boundednessとbackpressureを設計へ明記する |
| acquire deadline | timeoutはslot admission待ちの期限。finite／無期限／0をどう表すか。未admittedの取得は外部SQL副作用なし、begin admitted後の取消はcleanup責任をworkerへ残す |
| busy timeout | SQLite lock待ちの値、設定失敗のResult。古い500msを流用しない。busy timeoutはSQL全実行時間の上限ではない |
| begin mode | Deferred／Immediate／Exclusiveから呼び手が明示。rusqliteのdefault Deferredを選んだことにしない。WALやjournal policyはこれとは別 |
| close deadline／範囲 | active handleを待つかabort要求するか、timeout後のclosing状態、取消されたclose Futureの責任。推奨は新取得停止＋既存session終端待ち、強制abort／killなし |
| statement error | 通常SQL／decode ErrでもTxを継続可にするか自動abortするか。推奨はnative Txがactiveなら継続可、SQLiteが自動rollbackしたらterminal Aborted。後者を普通のstatement Errとして継続させない |
| retirement | capacity縮小継続／Pool全体取得停止／自動replacementのいずれか。推奨は取得停止し、pending acquireへ明示failure。既存active sessionは通常終端を許す。自動retryなし |
| user SQL範囲 | one-statement、transaction-control／savepoint／PRAGMA／ATTACH／DETACH禁止、DDL・virtual table・extension・SQL関数の許可範囲。以下のAuthorizer案を確認する |
| operation／commit deadline | 初版で新しいSQL interrupt／commit timeoutを導入するか。推奨は導入せず、caller取消とDB完了を分ける。追加するなら送信後結果不明・interrupt競合・worker責任を別承認する |

`pool容量/acquire/busy/mode/module`未決は既にStop（`docs/internal/compiler-rust-boundary-plan.md:159`、`open-questions.md:48–49`）。上表は必要な判断を追加で露出するもので、選んだ数値・defaultではない。

## raw SQLでTx終端を迂回させない案の比較

| 案 | 評価 |
|---|---|
| keyword／先頭token blacklist | 不採用。コメント・複数文・quoted text・schema trigger等のSQLite文法を自作する。通常stringを禁止語で拒否してもcontractの証明にならない |
| Statement.readonlyだけ | 不採用。SQLite header `:4706–4717`はBEGIN／COMMIT／ROLLBACK／SAVEPOINT／RELEASE／ATTACH／DETACHもreadonlyとなる場合を明記。ファイル変更有無はTx終端／connection設定変更ではない |
| 実行後is_autocommitだけ | 不採用。`INSERT; COMMIT; BEGIN`で最後のactive状態を見ても、先のcommit済みデータは戻せない。状態確認は防止に加える検査でありoutcome証明ではない |
| raw SQLを新Tx APIから全廃し、typed専用DMLのみ | authorizerが不要になる可能性はあるが、SQL／固定bind／FromRowの既存方向から別の公開APIへ広げる。ユーザーの代替判断が必要で、暗黙代替にしない |
| **safe Authorizer＋SQLite一文prepare＋専有worker＋状態検査** | 推奨。runtimeのhooks追加、SQL受理範囲の承認、SQLite反例の実験が必要。新driver／unsafe／SQL parserは不要な候補。静的opt-inとは別の常設runtime制約 |

推奨案の利用者区間ではTransaction／Savepointを全拒否し、Pragma／Attach／Detachも拒否する。Unknown／未認識actionはDeny。通常DML／必要なordinary DDL／Read／Select／既存SQLite組み込み関数等は、受理するactionを明示的に列挙する。新Poolはraw Connectionやnative SQL function／extension／virtual-table登録を公開しない。一般SQL sandbox・外部Rust本体の解析・DBファイルの悪意あるschemaへの包括保証は追加しない。

PRAGMAのread-only値参照まで拒否することも**新APIの明示policy候補**であり、旧Dbへ適用しない。table-valued `pragma_*`、view／trigger内の作用、automatic reprepareはprototypeで検査する。direct Pragma actionの拒否だけで、全pragma関連形・隠れたconnection mutationまで遮断できたとは宣言しない。承認したSQL制約をsafe hooksで満たせない形が出れば、具体反例を記録してStopし、許可範囲を変える判断へ戻す。

Authorizerはprepare時のhookで、一つのconnectionに一つだけ。schema変更によりstep時reprepareも起きるので、利用者statementのprepare／bind／step／finalizeまで正しいhookを保持する（SQLite header `:3379–3394,3442–3455`）。新Pool最小prototypeでは利用者statement cacheを使わず、native終端SQLとのcache混在を避ける。旧Dbのcache32は維持する。cache最適化はpolicy境界・schema reprepareを検証してから別に行う。

native BEGIN／COMMIT／ROLLBACKはhardcoded worker commandだけが実行するprivate区間とし、SQL文字列や外部handleからその区間を選べないようにする。hookを一時解除／切替するなら、unwind・rollback fallback・statement破棄の順をRAIIで検査する。管理区間中にuser SQL／FromRow callbackを実行しない。COMMITがUnknownへ写る現versionの制約を隠して、全Unknownを一般許可する案は採らない。

## 自動rollback・結果不明・closeの扱い

Authorizerで明示終端SQLを拒否しても、SQLiteの`INSERT OR ROLLBACK`、triggerの`RAISE(ROLLBACK)`、FULL／IOERR／NOMEM／BUSY／INTERRUPT等による自動rollbackは別に存在する。SQLite header `:6952–6957`はエラー後のautocommit確認を要求し、`:7148–7152`はconstraintによるimplicit rollbackを明記する。

各operationのstatement／row／iteratorを破棄した後、専有workerでnative stateを確認する。Txが予定外に終了したらsessionをterminal Abortedへ移し、そのhandleから後続SQLをautocommitで実行しない。lease generationを照合し、stale commandが次のsessionへ届かないようにする。自動rollbackを検出した接続の再利用は、残存statement、authorizer設定、cleanup、worker健全性を確認してから判断する。確認不能ならretire。is_autocommit=trueだけで「commit成功／データ整合／全設定復元」とはしない。

| 事象 | 推奨する観測と結果 |
|---|---|
| 普通のconstraint／row decode Err、Tx active | primary failureを返す。初版候補は継続可。statementがどこまで変えたかをErrからrollback済みと断定しない |
| 予定外のnative Tx終了 | Aborted／terminal session。後続操作拒否。自動rollbackの実row oracleとcleanupを確認 |
| commit成功＋reply受信 | Committedの完了結果。必要cleanup failureが別にあればcommit済みと接続retireを同時に表す |
| commitのnative Err | primary causeとcleanup causeを保持。fallback成功でもcommit失敗を消さない。outcomeが確定しないケースはUnknownとして区別 |
| COMMIT admitted後にreply喪失／caller取消／timeout | workerは終端責任を継続。呼び手にはcommit済み／rollback済みのどちらも断定しない。再実行しない。取消されたFuture自身へ架空のErrを届けたとは報告しない |
| rollback／cleanup失敗 | 接続retire、取得停止、waiting acquireへfailure。自動replacement／retryなし。Drop回数やpermit DropだけでIdleへ戻さない |
| worker panic／停止 | unwindで所有値を解放してもrollback成功とはしない。retire／Pool取得停止。未配達replyと未知outcomeを区別。abort／OOM／process kill後の普遍回復は保証外 |
| close deadline超過／Future取消 | closingは解除しない。close成功ではない。accepted SQL／cleanupはworker責任として継続。active Txをkillしない。次回closeの完了待機を許す候補 |

COMMIT結果とcleanup結果は独立軸。`Committed + Retired`、`NotCommitted + CleanupFailed`、`Unknown + Retired`を単一の「失敗＝rollback済み」に潰さない。現Errorはmessage中心なので、この情報を機械判別できる新Failureの表現選択がStopになる。Result／取消／panic／infraを混同しない。

## 最小safe prototypeの構造（まだ実装しない）

1. Pool ledger: Open／Closing／Failed／Closed、各slotのlease epochとReserved／Beginning／Active／Cleaning／Idle／Retired。connectionはworkerだけが所有する。idle通知はcleanup完了から送る。
2. worker lexical session: `transaction_with_behavior(explicit mode)`をそのscopeで保持し、typed commandsをblocking_recvする。Connection＋Transactionの自己参照保存、borrowed値のreply、任意FnOnceへの管理SQL権限の委譲をしない。結果はowned Send+'static。
3. session channel: 外側nonClone Txだけがclient senderを持つ。ledger／workerがsenderの強参照を保持しない。最後のsender Drop後はaccepted commandsを処理し、未終端ならworkerがrollbackする。Dropのtry_send(rollback)失敗へcleanupを依存させない。
4. Begin reply失敗: oneshot sendで戻った未配達Txをdropし、workerがcleanup。reply成功直後のreceiver取消も同じ経路。Future側permit DropだけでIdleへ戻さない。
5. 明示終端: send前取消とadmitted後取消をledgerで分ける。終端consume後のnative Drop fallbackも観測し、cleanup／state確認成功でだけIdle。panicを捕捉して続行するならworker connectionをretireする。
6. Close／Join ownership: workerに自身をjoinするInnerの最後のArcを持たせない。新Pool Dropは閉鎖要求の候補、明示closeはworker exit／connection close／joinを観測する。close timeoutがOS threadのkill／完全終了を保証する設計にしない。旧Dbの現在のDrop/joinを変更しない。

これはborrow checker未確認の構造案。Authorizerを解除／切替したままunwindする経路、native rollback fallback、close中のactive session、Dropによる最後のArcの所在を小prototypeで確認する。unsafeやruntime全面rewriteが必要になる場合はStop。

## 最小testsと実行gate

- **SQL制約:** 実SQLiteでCOMMIT／END／ROLLBACK／BEGIN／SAVEPOINT／RELEASE／ROLLBACK TO／PRAGMA／ATTACH／DETACHを各exec/query入口へ渡す。大小文字・コメント・引用内の語・複数statement・prepared reuse／schema reprepare・pragma TVF／viewを含める。禁止SQLはprepare／step時の拒否とrow不変を確認し、普通の文字列`'COMMIT'`は許容する。keyword normalizer／skipで通さない。
- **自動rollback:** UNIQUEに対するINSERT OR ROLLBACKと、実triggerのRAISE(ROLLBACK)を使い、Aborted後のSQLが保存されないことを別connectionで確認。BUSY／interrupt／decode Errは別oracle。triggerが初版SQL受理範囲外なら、既存schema triggerを含むDBの扱いを明示して検証する。
- **leaseと取消:** acquire前後、BEGIN実行後／reply前、operation admission後、commit admission前後／reply前、unpolled Future、Pending中Dropの正のbarrier。capacity=1でcleanup中に同slotを次beginが取得しないこと、cleanup後のrow可視性、epoch／worker終了／permit解放を確認。sleepだけの成功oracleにしない。
- **失敗:** private test seamでrollback／hook restore／connection close失敗・worker panicを注入。retired slotの不再利用、waiting acquire failure、close timeout→再待機、primary＋cleanup cause／unknown outcomeを観測。公開故障注入APIは作らない。
- **checker／native:** High・保存Low・UserLowでTx reuse／copy／shared／JSON／field保存／Option／Result／async alias spawn／標準task境界を検査。同一task awaitとowned委譲を実Cargo正例にする。capture-free関数署名の型言及と実捕捉を分け、emitterへ再推論を足さない。
- **依存境界:** 承認後、runtime単独、SQL checkerなしcompiler、生成appの独立workspace、4 OSでsafe hooksが使えることを検証。既存Db CRUD／queue／取消／DropとSQL opt-inの有効／無効をbaseline比較する。

Phase 3 acceptance → 上記policy/API/featureの判断 → Phase 4 ADR固定 → failing tests／safe小prototype → 実装 → bounded conformance/fuzz／全関連CIの順。現時点でprototype成功・4 OS成功・cleanup保証達成を報告しない。

## 標準SQL opt-inと旧Dbは別の境界

既存SQL checkerはBuiltin identityのdb_*リテラルだけを収集（`compiler/src/sql_check/collect.rs:18–31`）し、schema上でprepareする。新module operationを追加しても自動で対象にならない。Phase 4で対応するならcanonical operation IDから既存engineへ明示adapterを足し、typed row／bind／元位置の同じ検査範囲を保つ。対応しない期間は新operationを未検査と正直に報告し、既存db_*検査済み件数へ混ぜない。

opt-inは実DB接続・query実行・取消／Tx完了の証明ではない（`docs/sql-check.md:48–64`）。compilerのschema/query Authorizer allowlist（`sql_check/engine.rs:112–135,190–212`）を、そのままruntimeユーザーSQL policyに流用しない。新Txの常設Authorizerは動的SQLにも必要で、opt-inで先に成功したことを実行時防護の代わりにしない。

旧Dbはqueue64、busy500ms、cache32、exec複数文、caller取消後もaccepted job実行、最終Dropでjoinの現在契約を維持する（`runtime/src/database.rs:38–42,61–75,103–124`、`docs/database.md:65–75`）。旧BEGIN／COMMIT SQLを新制約で拒否したり、既存取消をrollback保証へ変えたりしない。

## 判断待ちとして残すStop

1. 新API／Failure／Parameters／Pool clone・close／全policy指定の形が未採用。数値defaultや新resource capabilityを勝手に選ばない。
2. safe Authorizer案のruntime `hooks`有効化は依存feature変更。ユーザーがこの具体変更を承認するまで追加しない。
3. raw SQLの受理範囲・一文制限・PRAGMA等拒否・自動rollback後のterminal state・retirement policyは新APIの公開契約。既存Dbと分離して判断する。
4. 方針をsafe native APIだけで満たせない反例、一般effect/region解析、unsafe、追加driver/parser/featureが必要なら停止し、狭い代替APIとの比較へ戻る。

この文書は実装の許可・依存feature追加の許可を既成事実にしない。未実行の候補を、既に成立したTx／Pool保証と書かない。

## 追記: 新Pool/TxのParameters境界（未採用・判断待ち）

2026-10-05。公開設計は汎用DB引数について任意個の型付き値、NULL、str/bytes/viewの保持規則を判断対象としている（`docs/library-design.md:58–74`）。現在の`query(id)`／`insert(name,age)`は旧Dbの固定形（`runtime/src/database.rs:130–188`）であり、新しい標準APIのschemaとして昇格させない。

| 比較 | 評価 |
|---|---|
| A: 旧固定bind継承 | 実装差分は小さいが、任意schema・型・個数に対応せず、demo由来のname/ageを新標準へ固定する。旧Db互換性の保持に限り、新Pool/Txの推奨から外す |
| B: owned opaque Parameters＋型別builder | **推奨候補。** runtime内部だけで`Vec<rusqlite::types::Value>`を所有し、Nagiには不透明resourceと明示operationを登録する。可変長文法、reflection、任意trait solverを要求せず、異種のbindを任意個組み立てられる。新resource/APIの判断は必要 |
| C: class ToParams derive／新trait solver | field順・NULL・型変換・derive対象・generic制約という別の公開契約を増やす。既存FromRowは出力側の対応であり、入力側のToParams承認を含まない。Phase4最小構成には採用しない候補 |
| D: zero-bindのみ＋Rust adapter | 新Parametersなしで狭く始められるが、一般のbindはRust作者へ委譲する。SQL文字列連結を代替にしない。利用者が機能縮小を選ぶ場合の比較案で、汎用標準API達成とは扱わない |

Bでは空のParametersを作り、`bind_i64`／`bind_f64`／`bind_text`／`bind_bytes`／`bind_null`等の**仮名**operationで各1値を追加してowned Parametersを返し、query/all/execへ最後にmoveする候補。mutable view、borrowed ValueRef、Nagiに公開するValue enumは初版へ要求しない。textの`str`とbytesの`bytes`はmoveし、それぞれString／Vec<u8>を保持する。Parameters自体もconsume→返却にすれば、内部Vecを借用したままawaitする経路を作らずに済む。bind順はbuilder呼出順・SQLのparameter index順とし、列名やname/ageから推測しない。SQLiteの番号欠番・同名placeholder再使用・named placeholderの受理範囲は明示判断する。

rusqlite 0.40.2のValueにはNull／Integer(i64)／Real(f64)／Text(String)／Blob(Vec<u8>)がある（vendor `src/types/value.rs:9–20`）。`params_from_iter`はfeature gateのないsafe APIで、Valueのowned iteratorまたは参照iteratorを受ける（`src/params.rs:429–452`、`src/types/to_sql.rs:327–331`）。bindingは個数不一致をErrにする（`src/statement.rs:474–493`）。この内部表現だけなら追加crate・version・feature・unsafeを要求しない。既出Authorizer hooksのfeature Stopは別途残る。SQLite Value::Nullは型タグ付きNULLではないため、plain nullを初版で許すか、明示したtyped-null builderを設けるかはAPI判断。i64範囲外整数、bool変換、NaN/Infinity、日時/UUIDの自動変換も黙って追加しない。

新resourceはnonCopy・暗黙cloneなし、generic parameterなしの候補。storage/shared/debug/Serde/task-transfer等は必要な各capabilityを判断して登録し、Vecだから許可できると推定しない。resource/operation identity、型別Passing、owner保持、High/保存Low/UserLow、独立registry期待の追加が必要になる。現在のResourceInfo公開shapeと既存22resource/47operationの意味は維持し、新項目追加後の集合期待は理由付きで増やす。登録名・builder名・Failureのbind/decode分類も未採用Stop。

allocationは空Vec自体のheap不要、追加時のcapacity確保/成長、通常のtext/bytes生成、SQL動的文字列の所有化、job/reply/Future、row/list出力を分けて測る。既存String/VecをValueへmoveする境界でdeep cloneは不要だが、文字列literalの通常生成にはallocationがあり、SQLite binding時も非空text/blobはSQLITE_TRANSIENTでcopyされる（vendor `src/lib.rs:317–336`、`src/statement.rs:643–665`）。zero-copy／allocationゼロ／固定個数を保証しない。既存cost reportは動的allocation数ではなくsitesで、builderも現時点では対象外（`compiler/src/emit.rs:2053–2055,2106`）。測定・cost表示の追加も新操作を見落とさない計画に含める。

SQL文字列はliteralをSql::Static、他をSql::Ownedへ渡す既存の判断をcanonical新operationへ適用する候補。現sealed factはBuiltin callだけに`sql_static`を作り（`compiler/src/check/checked.rs:622–653`）、emitもdb_*だけを変換する（`compiler/src/emit.rs:695–710`）。新module登録だけでは適用されない。新操作のsealed planに明示して、aliasでも同じidentity、引数評価順、動的SQLの所有化が後続のParameters moveより前に完了すること、Future/workerがSQLとParametersを所有することを検査する。既存Dbの変換は変更せず、emitterの綴り照合や再推論へ戻さない。

SQL opt-inは現collectorの固定bind_countを流用してBへ0/1/2/3を捏造しない（`compiler/src/sql_check/collect.rs:24–31`、`sql_check/engine.rs:542–546`）。canonical新operation用adapterは必要。動的Parametersの長さ/型を追うgeneral solverは持ち込まず、静的に証明できないbind検査は未検査と明示する。リテラルSQLのschema/返却列だけ検査する部分対応なら、bind件数と報告protocolを分離する判断が必要。全検査済みと表示しない。runtimeでは全入口でsafe bind個数検査、値型/NULL/FromRow decode失敗をFailureへ返す。opt-inはruntime policy、Tx完了、データNULLの保証を代替しない。

承認後の最小oracleは、異種0/1/複数bindと順序、NULL・空text/blob・埋込みNUL・Unicode・整数端、少ない/多いparameterと番号欠番、move後再利用拒否、SQLとbind textが同じ変数を参照する評価順、未poll/queued/pending取消時のowned payload Drop、動的Parametersのopt-in未検査表示と旧Db結果不変。prototypeやテストは未実行。新API／Parameters／Failure／各policyの承認とPhase3 acceptanceより前に実装を開始しない。
