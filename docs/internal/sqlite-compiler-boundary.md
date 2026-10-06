# SQLite compiler境界: 実装前の縦切りレビュー

2026-10-06。根拠は[ADR 010](adr/010-sqlite-transaction-boundary.md)、[API契約](sqlite-pool-proposal.md)、承認契約commit `3cce2f9`。これは将来の配線案と予定fixtureであり、標準APIの実装・受理・検証成功を示さない。現在`std.db.sqlite`はregistryに登録しない。一接続native coreの検証とwrapper adapterの比較後、runtime public APIとcompilerを同じ縦切りで接続する。

## 予定入力と検証段階

[fixture matrix](../../compiler/tests/fixtures/sqlite-contract/matrix.json)は15組のHigh／手書きLowを収録する。意味論は全件`planned_unwired`で、semantic harness未配線、check／Rust build／run未実行。[parse-only harness](../../compiler/tests/sqlite_contract_inputs.rs)はpublic parserを直接呼び、全入力の構文、matrix相対path／集合完全性、元行anchorとlexer token開始位置を検査する。2 testをローカルで実行して成功した。現在の未知module拒否はTxやSQL契約のREDではない。Lowは`import ... as ...;`を使う独立した利用者入力で、生成Low goldenではない。将来の保存Lowはchecker factsを持ち越さず、Highから生成して独立再load／checkする。

| 入力 | 将来期待 | 主な観測 |
|---|---|---|
| 01 same-task-affine | pass | 同task await、owned委譲、local Option／ResultからTxを取り出してcommit |
| 02 pool-row-sql | pass | Pool field／shared state、標準行型、SQL Static／Owned、動的Parametersのbind未検査 |
| 03 user-type-names | pass | user class Pool／Tx／Options／FailureとSQLite resourceの区別 |
| 04 pointer-signature | pass | `fn[sqlite.Tx, unit]`のpointerをspawn捕捉してもTx実payloadはない |
| 05 consume-reuse、06 copy-tx | fail | 終端後reuse、非Copy resourceのcopyをcheckerで拒否 |
| 07 owned-field | fail | owned／Option経由のfield保存を元field行で拒否 |
| 08 nested-shared、09 native-shared-context | fail | wrapperや標準Appの内部Arc contextを経由するTxを拒否 |
| 10 spawn-owned、11 spawn-alias-option | fail | 実引数Tx、async関数alias＋Option[Tx]のFuture捕捉をspawnで拒否 |
| 12 future-storage、13 future-option、14 future-return | fail | 既存Future保存／container／返却制限を維持。捕捉検査の成功証拠とは別 |
| 15 new-row-restriction | fail | 新allの未対応field拒否と、旧db_allのclass／手書きFromRow経路の維持 |

matrixの`variants`は追加実行を要する派生検査の予定であり、15組から実行済みtest件数を増やして数えない。01／02のnative build／runには完成したruntime public APIを必要とする。旧Db baselineは既存`sql_check`、`nullable_database`、`owned_database`等を維持する。

High／Lowの実形の根拠は`parser.rs`のimport／type／statement処理、`emit.rs::low`のrecord、`case Some/None/Ok/Err { ... }`、`scope { ... }`、既存`scope_runtime_contract`の手書きLow。fixtureの構文成功は上のparse-only範囲で確認した。公開表示位置はsource loaderの元pathと宣言／statement行を期待する。`source.rs::diagnostic`は行単位で、High expression columnを保証していない。matrixのUTF-8 columnはfixtureレビュー用anchor位置のみで、公開診断保証ではない。

## canonical importとresource registry

`stdlib.rs`の`StandardModule`へSqliteを追加し、module IDは`stdlib:std.db.sqlite`、native namespaceは`::nagi_runtime::sqlite`にする。resourceはPool、Tx、Parameters、Options、BeginMode、Failure、FailureKind、Outcome。Rust側enum識別子は既存Optionsとの区別のためSqliteOptions等でよいが、公開名を変えない。18操作の署名／Passing／Failure accessor／定数はAPI契約の表を独立した手書きinventoryとして検査する。

現`stdlib::resource`／`operation`は`modules::symbol(DefId)`と完全一致する。`resource_named`はmodule ID＋公開名から検索し、`Checker::resource`は登録definitionも確認する。この経路を保ち、`Type.0 == "Tx"`、末尾名、全局予約名でresourceを判定しない。例03のuser `class Tx`はfield保存／shareを許し、`sqlite.Tx`だけがnative契約を持つ。SQLite importを除いた同じuser class群、別user moduleのTx、同名関数queryもbaselineに加える。

public `ResourceInfo`のshapeは変更せず、private `ResourceContract`へTxのtask-transfer禁止を表すlifecycle／predicateを追加する。旧resourceは現在の`Unspecified`のまま。所有値は既存move検査、copy拒否は既存native copy判定、field／enum保存は`check.rs::class_field`の`storage=false`を使う。local Option／Resultをfield保存と同一扱いで拒否しない。Serde、Debug、shared、Charge、task-transferを単一trait solverへまとめない。

registry着手前に次のcapability oracleを具体値へ固定する必要がある。ADR／API表から明示できるPool Debug可、Parameters Debug不可、Tx nonshared／storage不可、Options shared不可、全新resourceのSerdeなしは推測で変更しない。

- TxのDebug、OptionsのDebug、FailureのDebug／shared／equality。
- BeginMode／FailureKind／Outcomeのstorage／shared／Debugと、表で明示されていない各資源のequality。
- Options／Failure等の未明示capabilityを既存類似resourceから一括trueとしない。`print(options)`や`share(failure)`の受理は明示oracleが必要。

これは未確定値の列挙であり、Tx/task捕捉の承認済み禁止を弱める判断ではない。

native owned payloadも承認済みTx field-storage禁止の観測対象である。`class_field`はuser class／enum fieldに効くが、local `actor.turn(tx, reply)`はTurnのinline state fieldにTxを保存しうる。Actor State SはMessage／Reply／EのCharge検査とは別で、現在Sへstorage検査を適用していない。registry配線時にはTurn／Actor Stateの実payload fieldへのTx永続保存も拒否する。function pointer署名はpayloadではなく、local Option／Result、owned関数委譲、同task awaitは許す。この観測対象の固定を新公共保証や一般effect解析の追加として扱わない。

## shared payloadとFutureの実捕捉

現`check.rs`のshareは直resourceの`shared`だけを確認する。`share(some(tx))`や`shared[Option[Tx]]`、`App[Option[Tx], E]`の内部stateはそれだけでは止まらない。`capabilities.rs::payload_any`と同じ反復遍歴を用途別に使い、新SQLite資源のnonshared payloadを検出する。入口はshare、`valid(shared[...])`、native `SharedPayload` roleの型引数。class／enum／owned／Option／Result等を辿り、function pointer signature、NominalPhantom、CallbackSignature、ActorのIndirectProtocolを実shared payloadと誤認しない。旧resourceのnested shared受理範囲を同時に変えない。

現Futureには保存可能な値／closure環境がなく、直接async呼出かそのlocal関数aliasから作る。`Var.async_function`は元named functionを保持し、aliasの別async関数への再代入を拒否する。既存`emittable`はFuture local保存、container、引数／戻り値を拒否する。これを維持したまま、以下のprivate factsで捕捉を明示できる。

1. `check.rs::expr_mode`でFutureを生成するCallを検査した時、解決済みcallee identity、実引数index／型、元`Span`、必要なowner `BindingId`を記録する。signatureだけを捕捉として読まない。native operationにはPassingも記録する。
2. 専用payload traversalで実引数中のcanonical Txを検出する。async fnは未poll時も引数をFuture frameに保持しうるため、bodyがTxを使わない場合も禁止する。
3. `S::Spawn`はFuture生成式の捕捉factを検査する。既存view task拒否を維持し、owned／Option／Result／owned[Tx]のTxを追加拒否する。拒否場所はspawnの元行、関連causeはcaptured引数と元binding。
4. `E::Await`の消尽済み子Future捕捉を外側Futureへ無条件unionしない。`spawn child(try await begin(...))`は外側の実引数Txを検出する。子task自身が内部でbeginする場合はcallerのTxを転送していないので許す。

最小配線は`checked.rs`にprivate `FinalCheckFacts`を置き、private `check_mode`／`integrate_mode`がfunction別`ExprUseId` mapを返し、`finalize`から`CheckedProgram::seal`へ渡す案。public check／integrateはfactsを破棄して現APIを保つ。public Exprへ任意の安全宣言fieldを足さない。finalizeは現在もnative統合後に最終checkするので、保存Lowと手書きLowでも同じ再構築入口を使える。

sealは必要なFuture生成／spawn factsの集合完全性、callee identity、引数index範囲と式への対応を検証し、欠落をcompiler defectとして拒否する。emitterは捕捉意味論を名前listで再実装しない。private unit検査でfact消去、wrong callee／index／owner、stale source spanを拒否し、Low再checkで再構築する。loop再check／branchで同じ式の事実を決定的に置換し、別式・別moduleのtoken範囲を混ぜない。一般effect／region解析、closure環境推論、trait solverは不要である。

## 行型、SQL所有化、opt-in schema

`check.rs::standard`から専用`sqlite_standard`へdispatchし、型hint／Passing／arityの検査は既存形式で行う。query／allはclass型に限り、`checked.rs::from_row`の標準生成可能判定を共通helperに移し、新APIだけで呼ぶ。現対象はstr／bytes、i8/i16/i32/i64、u8/u16/u32、f32/f64、boolと一層Option（ownedは同一表現）。u64、nested class、enum、resource、List等を受理してrustcへ送らない。旧db_*はclass-only検査とRust bridgeの手書きFromRowを保持する。scalarをuser classでshadowした場合もcanonical解決後の型で判定する。

`checked.rs::ExpressionPlan`へ新SQL操作用のprivate `SqlArgumentPlan { index, representation: Static/Owned }`を足し、既存db_*の`sql_static`planは維持する。canonical query／all／execの引数1だけを対象にし、sealでoperation identity／引数範囲を検査する。`emit.rs`のstandard-operation生成はそのplanを読み、literalは`Sql::Static`、その他は呼出時`Sql::Owned`にする。SQL所有化は後続Parameters moveより前に完了させる。Passing::Referenceだけを根拠にworkerへSQL viewを渡さない。

関係する検査はSQL literal／変数／view／alias、同式内Parameters評価、Err／panicの評価順、High／保存Lowの生成一致、native build。型やmoveが通るだけではallocation／clone／Future frameへの影響を検証したことにならない。Parametersのtext／bytes builderはmoveし、SQLite側のcopyや出力allocationは別に残る。

`sql_check/collect.rs`は既存Builtin collectorを残し、Standard resolution＋`function_id`一致のquery／all／exec adapterを追加する。同名user関数を収集しない。内部Site／Queryのbind知識を`Option<usize>`等で明示し、旧operationは現固定数をknown、新Parametersは証明できない時unknownとする。最初は動的Parametersを必ずunknownとしてもよく、builder alias追跡の大型rewriteは要求しない。unknownを0と捏造せず、literalのschema／必要列は検査し、bind未検査を別に表示する。動的SQLはSQL未検査。

`sql_check/engine.rs`の新operation variantは一文をSQLite Batchで確認し、query／allはreadonly＋column_count>0、execはcolumn_count==0とする。匿名placeholderはsafe parameter metadataで検査し、named／numberedを拒否する。旧operationのSELECT／DML形、固定bind数、件数・出力を変えない。SQL engineの既存schema sandboxとruntime Tx authorizerは役割が違い、opt-in成功からTx／取消／実DB一致を推論しない。新execのDDLを準備できるためのoffline authorizer調整も新operation scopeだけでレビューし、SQLを実行してschema snapshotを変えない。

## native prototypeとwrapper adapter比較

private一接続coreは、lexical native Transaction、user authorizer、typed command、EOF cleanup、結果／cleanup outcome、native closeとworker joinを実SQLiteで確かめる契約oracleとなる。public Pool／Txの受理、multi-connection admission、全Future捕捉、clone共通closeが成立した証拠ではない。

[Rust wrapper比較](sqlite-pool-rust-reuse.md)では同じoracleへadapterを接続し、checkoutを未終端sessionの間保持できるか、取消後のcleanup完了を誰が観測するか、retire時の自動replacementを止められるか、native closeとthread joinを誰が保証するかを比較する。専用thread prototypeをpool algorithm採用の決定にしない。crate既定のrecycle health check／close Ok／size==0だけを契約達成としない。Q004のgeneric deadpool比較とcapability初版表は承認済み。予想外の追加依存・更新が必要なら、版／feature／transitive依存と具体差分を判断へ戻す。

## 実装開始・停止の境界

public registryを生やす前にruntime native APIの実在と上記capability oracleを揃える。受理後の生成RustがNagiで検出可能な型／move／lifetimeで拒否されればP1として元checker／planへ戻す。実DB SQL型／NULL／範囲／worker failure、依存infra、trusted Rust adapterの最終Send等を別段階として報告する。

Future保存／返却解禁、一般effect／trait／region solver、unsafe、旧Db受理縮小、未承認の追加依存、authorizerの具体反例で承認SQL境界を満たせない場合はStop。fixtureの期待をacceptやskipへ緩めて解消しない。public配線後は元位置付きnegative、positive native build／run、High／保存Low／手書きLow、seal integrity、SQL opt-in、旧Db baseline、runtime独立build、4 OS CI、既存fuzz／生成探索が必要である。

## native監査で区別したstatement Errと先行効果

2026-10-06の独立レビューで、private prototypeのAFTER INSERT triggerからpragma TVFを読む負例は、Pragma Denyを実観測しSQL Err／Tx ACTIVEを返した後も親INSERTの1行が残ることを確認した。`/tmp/nagi-phase4-session/08-prototype-tests.log`のREDは、追加testのcount=0期待が未採用の「全statement Errで変更0」を要求したためであり、禁止Pragma actionの成功を観測した結果ではない。root判断はREDを保全し、採用済みAuthorizer／ordinary ErrのACTIVE継続／明示rollback契約を維持する。新testのその期待だけを訂正し、Deny原因、native先行効果、明示rollback後0と次sessionでのreuseを別oracleで検査する。public registry／保証は変更しない。

独立Python SQLite 3.53.1の`:memory:`再現も、同じPragma DenyでBEFORE triggerは0行、AFTER triggerは1行、いずれもSQLITE_AUTH／Tx ACTIVEで、明示rollback後は0行だった。通常の許可DMLだけでも`INSERT OR FAIL ... VALUES (1), (1)`はUNIQUE Err後に最初の1行を残し、TxはACTIVEのまま継続できる。SQLiteの[FAIL契約](https://www.sqlite.org/lang_conflict.html)と[RAISE契約](https://www.sqlite.org/lang_createtrigger.html#the_raise_function)、bundled一次sourceの`OE_Fail`説明は、先行変更をback outしないことを明示する。[Authorizer契約](https://www.sqlite.org/c3ref/set_authorizer.html)はDenyで当該prepare等を拒否するもので、許可済み親DMLの先行効果すべてをundoする保証ではない。

direct prepare時の拒否／禁止管理操作の拒否における不変確認と、AFTER triggerのstep失敗時のnative先行効果を混同しない。後者をTx rollback前から0と表示しない。各statementへsavepoint／自動undoを追加する、AFTER triggerを一律禁止する等は公開SQL／終了policyの別判断であり、このレビューでは採用しない。この追記は根拠と検査の分離を記録するもので、未実行の修正test成功やcompiler捕捉／source mapping完成を示さない。
