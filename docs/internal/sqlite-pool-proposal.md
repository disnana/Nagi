# Phase 4: SQLite Pool・affine Txの初版契約

2026-10-05の提案を、2026-10-06にQ002の選択1として承認。**公開API・SQL制限・終了policyとruntime rusqlite hooksを採用済み。実装・検証の完了ではない。** [ADR 010](adr/010-sqlite-transaction-boundary.md)に判断を固定した。詳細根拠は[decision proposal](sqlite-pool-research.md)。同日のprivate一接続prototypeとローカル検証は[結果](sqlite-session-results.md)へ記録した。公開runtimeは2026-10-08に実装し、compiler縦切りと4 OSのacceptanceは別に検証する。一接続のprivate wrapper adapterは[比較結果](sqlite-adapter-results.md)へ分けて記録した。Phase 3 acceptanceは#80のCI成功とmain反映で満たした。

第一候補は新module `std.db.sqlite`（canonical ID `stdlib:std.db.sqlite`）、runtime namespace `nagi_runtime::sqlite`。既存Db、db_*、FromRow、Sql、Error、標準HTTP/Actor Optionsを変更・削除しない。新APIへ固定id/name/age bindを継承しない。新言語syntax、reflection、ToParams derive、generic trait solverは導入しない。

当初は内部pool/dispatchが未確定だった。2026-10-08の採用は下の確定差分と[公開runtime判断](sqlite-public-runtime-decision.md)に記録する。[既存Rust wrapperの比較](sqlite-pool-rust-reuse.md)では、deadpoolのowned checkout、tokio-rusqliteの専用worker、r2d2の同期poolと、狭いsession adapterを候補に残した。専用workerという説明をpool algorithmの自作決定とは扱わない。cleanupとcloseの観測まで同じ条件でprototypeし、責任とコードを減らせる実装を選ぶ。runtime hooksはQ002で承認済み。Q004で[generic deadpoolの比較試作とcapability初版値](sqlite-pool-adapter-decision.md)を採用した。内部実装の成立と公開APIの検証はまだ完了していない。

## 採用した資源と値

| 名前 | 所有・capabilityの第一候補 |
|---|---|
| `Pool` | nonCopy。共有ledgerを持ち、明示`clone_pool`だけでhandleを増やす。owned field保存・shared contextを許す候補。Serde/equalityなし。Debugは状態表示のみ。cloneすべてでclosing/failedを共有 |
| `Tx` | nonCopy/nonClone/nonshared/nonSerde、owned field保存・task転送禁止。通常localのOption/Result、同一taskのawait、owned関数委譲は許す。commit/rollbackがconsumeする |
| `Parameters` | genericなし、nonCopy/nonClone/nonshared/nonSerde/equalityなし。owned field保存と通常のowned移動を許す候補。Debug・内部値のfield公開なし。既存Actor message/replyのnative-resource拒否は維持し、Charge対応を追加したとは扱わない |
| `Options` | このmodule専用の検証済み設定。nonCopy、owned field保存可、shared/Serde/equalityなし。必要値はconstructorの全引数に必須。default_optionsなし |
| `BeginMode` | Copy/equality可のnative enumを登録済み定数で公開: `DEFERRED` / `IMMEDIATE` / `EXCLUSIVE`。beginで必須 |
| `Failure` | nonCopy、owned field保存可。既存CallError/WaitError同様のnative値で、利用者のclass literalで構築しない。Serdeなし。primary/cleanup causeと終端outcomeを別に保持 |
| `FailureKind` / `Outcome` | Copy/equality可のnative enum＋定数。Nagiの新enum構文やnative enumの網羅match機能を要求しない |

これらは**Q002で承認した新capability**。Pool/Options/Failureのshared・Debug等の最終値もADRへ固定する。Parametersの内部`Vec<Value>`はString/Vec/u8/i64/f64/Nullを所有し、Nagiのstr/bytesは既にString/Vec<u8>へ生成される。move builderで実装可能性があり、既存非Copy resourceのcopy拒否を使える。Rustの最終Send/Syncは独立runtime/生成appのbuildで検証し、無条件のunsafe implを加えない。

## 採用したmodule操作

下表の`R`はPassing::Reference、`M`はPassing::Move。順序は引数順。一般のBorrowやHandler/Mapperは新SQL APIへ要求しない。既存OperationInfoの登録・canonical identity経由で解決し、public ResourceInfoのfield/戻りshapeを変更しない。

| 操作署名 | Passing |
|---|---|
| `options(connections: i64, queue_capacity: i64, acquire_ms: i64, busy_ms: i64) -> Result[Options, Error]` | M,M,M,M |
| `open(path: view[str], options: Options) -> Future[Result[Pool, Failure]]` | R,M |
| `clone_pool(pool: view[Pool]) -> Pool` | R |
| `begin(pool: view[Pool], mode: BeginMode) -> Future[Result[Tx, Failure]]` | R,M |
| `parameters() -> Parameters` | — |
| `bind_i64(parameters: Parameters, value: i64) -> Parameters` | M,M |
| `bind_f64(parameters: Parameters, value: f64) -> Result[Parameters, Error]` | M,M |
| `bind_text(parameters: Parameters, value: str) -> Parameters` | M,M |
| `bind_bytes(parameters: Parameters, value: bytes) -> Parameters` | M,M |
| `bind_null(parameters: Parameters) -> Parameters` | M |
| `query[T](tx: view[Tx], sql: view[str], parameters: Parameters) -> Future[Result[Option[T], Failure]]` | R,R,M |
| `all[T](tx: view[Tx], sql: view[str], parameters: Parameters) -> Future[Result[List[T], Failure]]` | R,R,M |
| `exec(tx: view[Tx], sql: view[str], parameters: Parameters) -> Future[Result[i64, Failure]]` | R,R,M |
| `commit(tx: Tx) -> Future[Result[unit, Failure]]` | M |
| `rollback(tx: Tx) -> Future[Result[unit, Failure]]` | M |
| `close(pool: view[Pool], timeout_ms: i64) -> Future[Result[unit, Failure]]` | R,M |
| `copy_primary_error(problem: view[Failure]) -> Option[Error]` | R |
| `copy_cleanup_error(problem: view[Failure]) -> Option[Error]` | R |

`Options`はconnections/queue_capacityを正の値、各msを有限の非負値として検証する候補。0msは待たない指定で、即時に条件が成立していれば成功する。acquireは空きslotを先に取得できるか確認し、closeは終了確認済みなら成功、busy=0も競合なしならSQLを実行できる。0なら無条件timeoutにしない。無期限・負数sentinelは初版に設けない。数値defaultや任意の上限値は作らず、native usize/Duration等への変換可能性とTokio channel/semaphoreの受理上限を検査し、範囲外はconstructorのErrにする。巨大な値を無条件に確保してvalidation済みとしない。queue_capacityは各worker/sessionのcommand inbox上限であり、全caller待機・全heapの上限とは宣言しない。acquire_msはslot予約待ち、busy_msはSQLite busy待ち、closeのtimeoutは完了待ち。SQL実行/commitの新interrupt deadlineは初版で導入しない。

openの初版は通常のfilesystem path、またはconnections=1の`:memory:`に限定する候補。`:memory:`を複数connectionで開くと別々のDBになるためconnections>1はvalidation Err。空pathによる接続ごとの一時DBと、`file:` URIは初版では拒否し、URIを解釈しないsafe open flagsを明示する。名前の自動rewrite、共有memory URIへの置換、WAL/journal設定の暗黙追加はしない。URI/共有memoryが必要なら公開policyの別判断へ戻す。

bind_f64は有限値だけを受ける候補で、NaN/InfinityをSQLiteのNULL等へ暗黙変換しない。bind_nullはSQLiteのplain NULLを明示する。型付きNULL、bool/日時/UUID等の追加builderはこの初版へ含めず、必要ならAPI判断へ戻す。bind順は追加順、SQLite parameter index順。初版のplaceholderは匿名`?`のみに制限する候補で、safe prepare後のSQLite parameter metadataでnamed/numbered形を拒否する。SQL文字列のkeyword heuristicで判定しない。

query/allのTは、現行Nagi DB同様にclassのみ（enum・scalar・resource・任意genericは対象外）。初版の標準行変換は現在生成FromRowが対応するscalar str/bytes/数値/boolとそのOption fieldに限定し、field名からcolumn indexを解決する。NULL/整数範囲/型不一致はruntime decode Err。新APIで未対応のclass fieldを受理してrustcへ送らない。既存Rust連携の手書きFromRow経路・旧Db受理範囲は保持する。query/allはsafe prepare後のSQLite readonlyかつcolumn_count>0という「読取専用でrowを返す一文」と定義し、VALUES等も含める。SQL構文をNagiで再parseしてSELECT keywordだけへ制限しない。queryは0行→None・最初の1行→Some、allは全行。execはcolumn_count==0の一文で、RETURNINGはcolumn検査で実行前に拒否する候補。readonly/column検査は操作形の検証であり、Txcontrol防護のAuthorizerを代替しない。row-countはそのstatementの直接変更数で、旧Db.execの複数文・trigger込みtotal_changesとは区別する。

Failureの公開field候補は`kind: FailureKind`、`outcome: Outcome`、`retired: bool`、`message: view[str]`。messageはFailure ownerから借用する既存FieldInfo形式。causeを必要とする呼び手は上の明示copy operationを使い、Errorのkind/messageを複製するコストを隠さない。FailureKind定数候補は`INVALID` / `CLOSED` / `ACQUIRE_TIMEOUT` / `BUSY` / `SQL` / `BIND` / `DECODE` / `ABORTED` / `CLEANUP` / `WORKER` / `REPLY_LOST` / `CLOSE_TIMEOUT` / `ALLOCATION`。Outcomeは`NOT_APPLICABLE` / `ACTIVE` / `COMMITTED` / `ROLLED_BACK` / `UNKNOWN`。NOT_APPLICABLEはTxがまだないvalidation/acquire/close等、ACTIVEはstatement Err後もnative Txがactiveと確認できた場合。Failureから既存Errorへの暗黙変換は足さず、必要なアプリで既存std.result.map_errorを使う。

## 保証と失敗時policyの候補

一Txは一connectionを専有し、session中は他のPool利用者へ渡さない。worker scope内のsafe rusqlite Transactionを用い、外側handleはowned sender＋lease epochだけを持つ。borrowed Connection/Transactionをreplyへ出さず、自己参照・unsafe・新runtime基盤を要求しない。Txのtask転送禁止には、direct/alias async callからspawn等への実捕捉を追うprivate factsが必要。Passingや戻りFutureの型だけで達成済みとしない。既存Future保存/返却制限は解禁しない。

commit/rollbackのhandle consumeはFuture作成時から有効。未poll、送信前取消、begin返信喪失、最後のTx Dropでもworkerがcleanupを所有する。queue fullで失敗するDrop.try_sendだけには依存しない。受理済みSQLは取消で戻さず、未終端sessionをworkerがrollbackする。statement/row/iterator破棄、native Tx終端、hook復元、worker健全性を確認してからだけslotを再利用する。Dropが呼ばれたことはrollback成功の証拠ではない。

普通のSQL/bind/decode Errではnative Txがactiveなら継続可。SQLiteの自動rollbackを検出したらAbortedへ移し、そのhandleの後続SQLを拒否する。rollback/cleanup失敗、worker panic、状態確認不能は接続retire＋Pool新取得停止＋waiting acquireへfailure。既存active Txは終端を続ける。自動replacement/retryをしない。COMMIT完了＋cleanup失敗は`outcome=COMMITTED, retired=true`のErrとして両方保持する。返信が失われれば呼び手は結果不明で、rollback済み・再実行可能とは判断しない。取消されたFutureに架空のErrを届ける保証もない。

statementのErrは「変更なし」でも「rollback済み」でもない。`INSERT OR FAIL`やAFTER triggerでのstep失敗は、先行する変更をactive Tx内に残し得る。Authorizerが禁止PRAGMAを拒否しても、親INSERTの先行効果を自動で戻す契約にはならない。明示rollbackと未終端session cleanupの完了は別に検査する。暗黙のstatement savepointや、全Errでの強制abortを追加しない。

closeはclone共通の新取得停止、active Tx/cleanup待ち、connection close/worker終了の観測。closeの処理開始後は、timeout/close Future取消でもclosingは解除しない。未pollのFutureだけで閉鎖開始を保証しない。active Txの強制kill/rollbackはしない。再closeで完了待ちを許す。Pool最後のDropは新取得停止と閉鎖要求までで、非同期cleanup完了の保証ではない。無期限blocking SQL、abort/OOM/process kill、trusted Rust adapter内部のspawnや外部副作用の普遍回復は保証外。

## SQL・依存・allocationの境界

新Txでは一文safe prepare＋常設Authorizerを使う候補。利用者SQLのTransaction（COMMITを含むvariant全体）、Savepoint、Pragma、Attach、Detach、UnknownをDenyし、Ignoreを使わない。初版は通常SELECT/DMLとordinary table/index/view/trigger DDLの必要actionだけを許可し、virtual table/extension登録・load_extension・raw Connection公開は含めない。keyword/is_readonlyは根拠にしない。管理BEGIN/COMMIT/ROLLBACKはworkerのhardcoded private区間だけから実行し、その間にuser SQL/FromRow callbackを実行しない。prepare/bind/step/reprepare/finalizeまでhookを保持し、新user statement cacheは初版に設けない。

SQLiteの自動rollbackは上の拒否でも残る。pragma TVF、既存trigger/view、reprepareでpolicyを迂回できないかを実SQLiteで検証する。safe hooksで承認制約を満たせない反例はStop。悪意あるDB schemaや全SQL functionに対する包括sandboxは承認案に含めない。

rusqlite 0.40.2は維持し、runtimeの既存`bundled`へ**`hooks` featureだけの追加をQ002で承認済み**。compilerのfeature合成だけに依存しない。safe Authorizerとsafe params_from_iterはprivate prototypeで実行したが、public APIとwrapperはまだ接続していない。Parametersはowned Vec<Value>をconsumeし、text/bytesをdeep cloneせず移す候補。Vec成長、literal生成、動的SQL所有化、SQLite SQLITE_TRANSIENTのtext/blob copy、job/reply、row/list出力は残る。zero-copy、allocation数不変、速度改善を保証しない。

SQL literalは新operationのsealed planからSql::Static、その他は呼出時にSql::Ownedへ所有化する候補。SQL引数の所有化を後続Parameters moveより前に完了し、既存評価順/alias/High/保存Low一致を検査する。Referenceという表示だけでSQL viewをworkerへ渡さない。

SQL opt-inは新canonical operation用adapterを追加する候補。literal SQLのschema/返却列検査とbind検査を分離し、動的Parametersの個数/型が静的に証明できない場合は**bind未検査**として表示する。動的SQLはSQL未検査。既存collectorの固定0/1/2/3を捏造しない。runtimeでは全入口でsafe bind個数・値型・NULL/decodeを検査する。opt-inの成功をTx/取消/実データ保証と扱わず、既存db_*の結果・件数を維持する。

## Q002で採用した3判断

1. **新APIと所有契約:** module/resource/署名/Passing/Failure表現、Parameters B、初版のclass行型・plain NULL・匿名placeholder・SQLite読取専用row文/columnなしexecという範囲、required Options/mode/timeoutと0msの意味、`:memory:`複数接続/URI拒否を採るか。数値defaultなし。旧Db/APIは不変。
2. **依存とSQL policy:** runtime hooks追加、および一文/Authorizer/管理SQL分離/PRAGMA等拒否を採るか。既存DB trigger等も含む反例検査で満たせなければ、保証を縮めず再判断する。
3. **終了と障害policy:** cleanup確認前reuse禁止、退役時Pool取得停止・retry/replacementなし、ordinary Errのactive継続、自動rollbackのAborted化、commit outcomeとcleanup causeの分離、close timeout後もclosing維持・killなしを採るか。

判断後にADR固定→小さいsafe prototype/failing tests→実装→High/保存Low/UserLow/native、取消/cleanup/reuse/SQL迂回のpositive barrier、旧Db baseline、独立runtime/生成app・4 OS CIの順。protoで借用、task捕捉、hook復元、Drop cleanupのどれかを満たせない場合は具体反例でStopする。private一接続の成功を、新public API・保証の実装成功と報告しない。

根拠: repository `compiler/src/stdlib.rs`のOperationInfo/Passing/constants/FieldInfo、`compiler/src/check.rs`のstandard operation検査・非Copy resource copy拒否・既存DB class要求、`compiler/src/check/checked.rs`のstr/bytes生成型とFromRow plan、`runtime/src/database.rs:9–18,120–191`、`runtime/src/lib.rs:28–40`、`docs/library-design.md:44–74`。依存safe API/SQLite actionとDropの行根拠は詳細decision proposalに記録済み。

## 2026-10-08の公開runtime確定差分

ユーザー承認に基づき、deadpoolの試作から既存Tokio Semaphore＋lazy専用adapterへ移行する。全connections分のslot配列を持たないためOptionsへprivate slot Layout上限を加えず、正数/usize/Tokio上限と時間表現、busy_msのi32境界を検査する。native ledger/idle queueの実増分はfallible予約で検査し、新ALLOCATIONはそのErrを表す。予約失敗をINVALIDや未起動WORKERへ混ぜない。OOM普遍回復を保証しない。

openはpathを所有して管理構造を作るlazy operationで、native Connection/worker起動はbegin時に行う。したがってfilesystem/native open不成立はbeginのWORKERで構造化Database causeを保持する。初版にeager prefillは追加しない。取得予算はSemaphore待ちからnative record登録までで終了し、0ms・登録後ready/BEGIN/busy除外を維持する。

Failureのcauseは既存Errorのkind/messageを保持し、Debugはkind/outcome/retiredだけを表示する。idle返却の予約失敗はDrop内で記録して停止・退役・実joinへ進む。Txがないopen/acquire/close失敗はNOT_APPLICABLEを返す。公開Rust moduleはruntime::sqlite一つで、private prototype oracleもこの本体を実行する。詳細は[公開runtime判断](sqlite-public-runtime-decision.md)を参照。
