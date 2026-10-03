# Supervisor

`std.actor`のSupervisorは、登録したactorやasync処理の起動・異常終了・再起動・停止を管理します。一つの子が再起動している間も、ほかの子は処理を続けます。

## 再起動方針

| 方針 | 再起動する場合 |
| --- | --- |
| `RestartPolicy.TEMPORARY` | 再起動しない |
| `RestartPolicy.TRANSIENT` | 子のErrorやpanic。actorの既定値 |
| `RestartPolicy.PERMANENT` | Error・panicに加え、正常に処理を終えた場合 |

再起動時には初期化関数を呼び直し、新しい状態を作ります。処理中や待機中だったメッセージを再配送しません。状態の復元が必要なら、初期化関数でDBなどから読み込みます。

既定の再起動間隔は10ms、回数はSupervisor全体で10秒間に5回までです。上限を超えると、ほかの子も停止して`run`がErrorを返します。明示的な停止や親のキャンセルは再起動の理由になりません。

## 所有と停止

`run`へSupervisorを渡すと登録が締め切られます。`Control`は停止と監視のためのハンドルです。Controlを破棄しても、所有されているSupervisorは動き続けます。Supervisorや実行中の`run`を破棄すると、子へ停止を要求します。

`shutdown`が成功するのは、子と共通データの後片付けが済んだ後です。期限内に終わらなければ未完了のErrorを返し、終了を追跡する記録を保持します。HTTPやnative処理が共通データを保持している場合も、その参照が解放されるまで後片付けは完了しません。Rust連携で別のTokio runtimeを使う場合は、`run`を開始したruntimeを後片付けまで維持します。

Tokioの停止は協調的です。yieldしないCPU処理、blockingなnative呼び出し、重いDropは強制中断できません。長い計算は分割して`await actor.yield_now()`を挟むか、終了を管理できる別プロセスで実行してください。共通データを持たずに始まった外部のblocking jobなどは、このグループの終了保証に含まれません。

## 監視

`next_event`で起動、失敗、再起動、停止を受け取れます。イベントは既定256件、診断文は最大1024 UTF-8 bytesです。読み手が遅れた場合は`EventKind.LAGGED`と失われた件数が届きます。監視の遅れで子の処理を止めません。

これは同じプロセス内のnative実装です。VM、無停止のコード差し替え、分散配置、永続mailbox、実行中の子の追加・削除は未対応です。

[actorの書き方](actor.md) · [APIリファレンス](actor-reference.md) · [性能測定](actor-performance.md) · [実行できるサンプル](../test-nagi-code/library-examples/supervised-service/README.md)

旧[supervisor.nagi](../examples/supervisor.nagi)は固定workerの再起動試験です。
