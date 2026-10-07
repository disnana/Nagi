# SQLite compiler境界: 公開縦切りの実装と検証

2026-10-08。[ADR 010](adr/010-sqlite-transaction-boundary.md)、[API契約](sqlite-pool-proposal.md)、[runtime実装判断](sqlite-public-runtime-decision.md)に従い、`std.db.sqlite`のpublic runtimeとcompilerを接続した。未リリースの実装であり、merge／release／version変更はこの作業に含めない。本書の2026-10-06版は配線前の計画であり、現在の実装・実行結果と区別する。

## canonical identityと用途別capability

`stdlib:std.db.sqlite`へ8 resource（Pool、Tx、Parameters、Options、BeginMode、Failure、FailureKind、Outcome）と18 operationを登録した。native namespaceは`::nagi_runtime::sqlite`。操作identityはstandard resolutionと`function_id`の一致、resource identityは登録されたDefIdと照合する。同名user class／functionをresourceとして扱わない。

private lifecycle predicateでTxのtask transferを禁止する。owned値の消費は既存move検査、class／enumのTx fieldはstorage検査で拒否する。local Option／Result／List／Mapの値移動は許可する。Actor Turnのinline stateにもTx保存検査を適用し、共有payload、native内部Arc context、generic copyをそれぞれ別の用途別predicateで確認する。function pointerの署名、callback signature、nominal phantom、indirect protocolは実payloadと混同しない。copy用途はshared境界で探索を止め、Arc複製をFailureの深いClone義務へ変えない。

Parametersはfield保存可・Debug不可。private ItemPlanにはDebug eligibilityと読みやすい表示名を別々に記録し、seal／emission validationでitem identityとfield graphを検証する。Debug不可のclass／enumにはderive／custom Debugを生成しない。Failureの共有可・metadata Debug可はSerde／equalityの許可ではない。registry testsはresource集合、18 operationの署名／Passing、generic role、accessorを独立した手書きinventoryと照合する。

Failureのpublic native fieldsには専用のprivate FieldRead planを使う。kind／outcome／retiredは値、messageは`as_str()`の借用として生成する。既存resourceのmethod accessorは保つ。message借用中のFailure moveはcheckerで拒否し、共有State内のFailure／Pool fieldの借用も実native入力で確認する。

## Futureの実引数とprivate final facts

`expr_mode`がFutureを返すCallのcheckに成功した時、function別ExprUseId mapへcanonical callee、解決種別、各実引数のindex／ExprUseId（lineとspan）、型、Passing、および参照されたlocal bindingのBindingIdを記録する。user async aliasは`Var.async_function`の元named functionを使用する。public Exprへ安全宣言fieldは追加していない。

private `FinalCheckFacts`は`check_mode`→`integrate_mode`→`finalize`→`CheckedProgram::seal`を通る。public check／integrateのAPIはfactsを破棄して従来の戻り値を保つ。spawnは記録済み実引数の型からTx payloadとview／native borrowを検査し、Taskの結果型にあるTx payloadも拒否する。関数署名のTxだけでは拒否しない。消尽した子Futureの捕捉型を外側Futureへunionしないため、`spawn child(try await completed(tx))`の外側実引数がunitなら許可する。一方、外側実引数がawait結果のResult[Tx, Failure]なら拒否する。

sealは同じownership checkerを最終ASTのcloneへ再実行してprivate factsを再構築し、受け取ったfactsとの完全一致を要求する。これは欠落factsの修復や上書きではない。missing fact、wrong callee／argument index／Passing／BindingId、stale argument span／call ExprUseIdはcompiler defectとして拒否する。別のowner解析を追加せず、checkとsealの判定規則を一致させるための再checkであり、finalizationでAST cloneとcheck一回のコストが増える。seal後の入力とfactsはprivateかつimmutableで、emitterにcapture推論や名前listによるownership判定を追加していない。

High、保存Low、手書きLow、native replacementはすべて最終checkからfactsを作り直す。loop／branch再checkでは同じExprUseIdを決定的に置換する。Futureのlocal保存／container／返却制限、既存Task scope／受取義務、旧view task拒否は維持する。一般effect／closure／region／trait solverやFuture保存の解禁は行わない。

## 行型とSQL引数の所有化

新query／allは標準FromRowを生成できるscalar fieldのclassを要求する。str／bytes、i8/i16/i32/i64、u8/u16/u32、f32/f64、boolと一層Optionが対象。u64、nested class、enum、resource、List等はcheckerで拒否する。旧db_*のclass-only検査と手書きRust FromRowの経路は変更しない。

新query／all／execのprivate NativeCall planには引数index 1とStatic／Owned表現を記録する。literalは`Sql::Static`、その他は`Sql::Owned(<str as ToOwned>::to_owned(...))`とする。SQLの所有化を後続Parametersの評価・moveより前に完了させ、workerへSQL viewを渡さない。planを生成する段階でcanonical operation identity、引数範囲、SQL型を検証する。emission validationは式から期待NativeCall planを再構築し、欠落／wrong index／wrong representationを拒否する。既存db_*のsql_static planは保つ。

native testではliteral、str／viewの動的SQL、Parameters move、SQL→Parametersの評価順、SQLのErr／panicでParametersを評価しないこと、NULL／blob／bool／u8の行decodeと範囲外Decode Failureを観測する。runtime copy、出力allocation、Future sizeの測定は別のcost harnessであり、型・moveの成功だけをコスト証明とはしない。

## opt-in SQL schema検査

collectorは既存Builtin経路を保ち、canonical query／all／execのみを追加収集する。同名user関数は収集しない。旧operationのbind数はSome(固定数)、新ParametersはNone（unknown）であり、0へ捏造しない。literalではSQL shape／必要列を検査し、bind未検査を`SQL bind unchecked`として報告する。動的SQLはruntime検査として表示する。builder alias追跡は追加しない。

新query／allはreadonlyかつ返却列あり、execは返却列なしを要求する。SQLite Batchで一文のみを準備し、匿名`?`はparameter metadataで検査し、named／numbered placeholderを拒否する。旧DbのSELECT／DML shape、固定bind数、numbered parameterの受理は維持する。exec専用のoffline authorizer phaseはDDLのprepareを許可するが、SQLはstepしない。準備したCREATE TABLEは後続検査のschema snapshotへ現れず、application rows／total_changesも変わらない。schema sandboxとruntime Tx authorizerの役割を混同しない。

CLI testsはHigh／独立保存Lowで、新SQLのunknown bind表示、動的SQL除外、必要列、placeholder、readonly、exec返却列、一文制限と元source行を確認する。既存SQL suiteも維持する。opt-in成功は実DBのschema一致、runtime値型、取消、Tx終了を保証しない。

## matrixとnative証拠

[fixture matrix](../../compiler/tests/fixtures/sqlite-contract/matrix.json)は23組（46入力）を収録し、statusを`validated_public_slice`へ更新した。[parser harness](../../compiler/tests/sqlite_contract_inputs.rs)はsyntax／集合完全性／元行anchorを検査する。[semantic/native harness](../../compiler/tests/sqlite_public.rs)は全High／手書きLowのchecker結果を確認し、16 negative組は生のchecker診断fragmentと元path／行を照合する。parse／resolve拒否をnegative成功へ数えない。

7 positive組（01、02、03、04、16、20、21）は、各組の関数を実際に呼ぶRust assertionを用い、High、High削除後に再loadした保存Low、独立手書きLowの3経路でgenerated Rustをbuild／runする。空mainを実行しただけの証拠ではない。real SQLite Tx終端、row／Parameters、user名、function pointer、Parameters field、shared Failureとmetadata Debugを観測する。matrixのvariants文字列は追加観測の説明であり、matrixの23組から実行件数を増やして数えない。

追加native inputではlocal Tx List／Map、Option[shared[Failure]]のcopyが同じArcを保つこと、Failure accessor、shared StateのPool／Failure借用、完了済みsubfutureの非unionを確認する。追加negativeはTxのTask結果、nested awaitから得た実Tx payload、Actor Turn State、Failure.message借用中のmoveをHigh／手書きLowのcheckerで拒否する。private unit tamper testsはFuture facts、Debug plan、SQL planの整合を別々に検証する。

local Linuxでの実行ログはsession artifact directory `/workspace/nagi-sqlite-public-2026-10-08`へ保存し、生成Low／Cargo JSON build出力／test出力を残す。focused lib 127、registry 4、SQL CLI 14、parser 2は成功。native matrixと追加caseの最終統合ログは`compiler-public-native-matrix.log`。この件数だけを品質の証明にはしない。4 OS CI実行、全回帰、fuzz、独立review、cost測定、immutable archiveの配布検査はtop-levelの完了判断へ別途統合する。このローカル記録だけで他OSやrelease済み機能を主張しない。

## runtime保証との境界

SQLのordinary ErrはTx ACTIVEのまま継続し、先行効果が残る場合がある。取消やpanic捕捉をrollback成功と表示しない。`INSERT OR FAIL`やAFTER triggerのstep Errから「全statement Errで変更0」を追加しない。runtimeのauthorizer、cleanup outcome、retire、close、joinの証拠は[runtime実装判断](sqlite-public-runtime-decision.md)と独立runtime testsへ委ねる。

compilerが受理したsupported NagiをRust型／move／lifetimeで拒否された場合はP1としてchecker／private planを直す。trusted Rust adapterの最終Send／Sync／Clone、link／target／依存infra、実DB値／NULL／schema不一致は別の観測段階として報告する。新依存、旧Db受理縮小、Future保存解禁、一般solver、unsafe、runtime全面交換はこのcompiler実装の判断へ含めない。
