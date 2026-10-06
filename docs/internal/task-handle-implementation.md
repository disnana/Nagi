# S1 Task結果handle: 実装の接続判断

2026-10-06。ADR 012の採用意味論を変えず、[Stage 1](task-bridge-stage1-results.md)のprivate runtime検証からcompiler接続へ進むための記録。Stage 1 source `6223ad2` の4 OS検証と再開時の最新head CIを確認してから、ユーザーの再開指示に沿ってcompiler/runtime本体へ接続した。今回の実装・回帰・測定・独立レビュー・CIは[接続結果](task-handles-s1-results.md)に記録する。公開SQLite、release、版更新は対象外。

## 最小APIと互換性

- `task = spawn work()`専用binding文を追加する。旧statement `spawn work()`、ユーザー関数`spawn(...)`、ユーザー型`Task`を維持する。裸名だけで標準operation/typeと判定しない。
- canonical `std.task.Task[T]`はscope所属の非Copy・非Clone・非shared handle。型identityへscopeやruntime ticketを混ぜず、checkerの私有factsで所属と未消費義務を持つ。
- `await task`は一回consumeし、実join後の`Result[T, TaskFailure]`。内側業務Resultをflattenしない。`std.task.discard(task)`はunitを返し、受取放棄を明示する。停止・join完了・故障抑制にはしない。
- `std.task.kind(failure)`はCopyな`TaskFailureKind`、`std.task.message(failure)`はfailureを借用元にする`view[str]`。KindはPanicked/Cancelled/LegacyError/Internalの四定数を既存のresource constant経路で公開する。TaskFailureはopaqueな非Copy・非Cloneの値、暗黙`From<TaskFailure>`は追加しない。
- Nagi scopeは引き続きruntime Errorを出口から伝播する。TaskFailureを受け取って処理してもsticky failureは解除しない。legacy primaryは元Error kind/messageを保ち、body元Errを後続faultで置換しない。

## compiler/checkerと生成

専用SpawnBind ASTを追加し、標準Task definitionは正式metadataで接続する。stdlibをimportしないbindingもcanonical型を得て、保存Lowへidentityを残す。受取、放棄、move alias、scope選択はchecked representationの封印済みplanから生成する。emitterで新たなownership推論、暗黙clone、名前によるoperation判定を行わない。

scope identityは構文の所属を表し、native ticketと別にする。loopの固定点再checkでも同じ構文scopeを安定して識別し、同じ深さ・同じ行の別scopeを同一視しない。Task義務はbindingのavailable/moved状態だけに代用せず、正常binding出口、正常scope出口、継続分岐、backedgeで確認する。moveの義務転送、await/discardの義務解除、裸move式での放棄拒否を対で検査する。

最寄りscopeにTask bindingがある場合だけsealed ScopePlanがTaskScopeを選ぶ。if/match/loop内は対象にし、nested scopeは独立して選ぶ。旧spawn-only scopeは旧runtime Scopeを維持する。TaskScope内の旧statement spawnはlegacy modeへ接続してfail-on-Errを保つ。外ScopeのTaskを内Scopeで受け取ることは拒否し、内Scope終了後の外Scope受取は許可する。

現在のscope bodyラベル、元のlocals cleanup anchor、Error変換を維持する。scope内returnを解禁せず、全面async wrapperへ置き換えない。Task型を引数/return/field/container/wrapper/他taskへ逃がす経路を拒否する。型注釈やcopy/share/borrowで所属義務を消せないことも確認する。

## runtimeとコスト

privateの唯一JoinSet owner・typed receiver・ticket・receipt・sticky causeをpublic接続へ移し、private/publicの同じ意味論を二重実装しない。fake native IDとDrop gateはtest seamに限る。TaskScopeのspawnはlegacy Future<Result<(),Error>>、spawn_valueはtyped結果、receiveはTaskFailure、discardはunit、join/cancelは実join後のErrorを返す。同期Dropはabort要求までである。

private試作のrecord毎の全entry sweepはO(n²)候補。対象ticketの退役とdrain終端の一回sweepで既存oracleを保てるか検証する。新observer、scope callback、独自GC、capacity、timeoutは追加しない。join済みhandleの外部Dropから次のScope操作まで軽量recordを保持し得ることと、HashMap capacity・関連faultの保持量を測る。

## 先行テストと進行条件

[契約入力](../../tests/task-handles/README.md)に加え、裸move放棄、両分岐消費、消費後再生成の合流、nested旧scopeから戻った外Task受取、同深さの別scope、Failure.message借用中moveを固定する。陰性例は拒否段階・元行・診断を確認し、parse/import失敗でGREENにしない。

High→保存Low→nativeと独立Low→nativeで、業務Errの兄弟継続、TaskFailure観測後のscope Err、取消要求後の全join、旧spawn混在、未受取/二重受取/escape拒否を確認する。runtime 16群も本体へ移して回帰する。

同条件の手書きRust＋同じTaskScopeとNagi生成Rustを測定し、必要ならbare Tokioを保証範囲の異なる参考として分ける。Future size、allocation/保持、処理時間、binary/compile timeを記録し、測定前にゼロコストや安全性を断定しない。全回帰・4 OS・日英Docs・Sol再レビューを完了してmain向けdraft PRの完成で止める。


## 停止と引継ぎ

Stage 1は自然な区切りで停止した。[Sol 6.1向け引継ぎ](handoffs/2026-10-06-task-bridge-stage1.md)は、その停止時点の記録である。2026-10-06のユーザーの再開指示を受け、main `9ba4a104`、PR #88 head `5c2c8282`、再開時のchecks/website成功を読み戻してS1を接続した。上記の具体API・scope選択・義務追跡を用い、追加RED、checker/public runtime、三構文native、回帰・測定・日英Docs・独立レビュー・4 OS CIを[接続の実行記録](task-handles-s1-results.md)へ残した。S1完成後のS2具体API、公開SQLite、merge/release/版更新は別工程とする。
