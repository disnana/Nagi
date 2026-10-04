# Supervisor

`std.actor`のSupervisorは、同じプロセス内に登録したactorやasync処理の起動・異常終了・再起動・停止を管理します。現在の再起動方式は、失敗した子だけを起動し直すone-for-oneです。ほかの子は処理を続けます。

## 再起動方針

| 方針 | 再起動する場合 |
| --- | --- |
| `RestartPolicy.TEMPORARY` | 再起動しない |
| `RestartPolicy.TRANSIENT` | 子のErrorやpanic。actorの既定値 |
| `RestartPolicy.PERMANENT` | Error・panicに加え、正常に処理を終えた場合 |

同じ失敗でも、ハンドラーのどこで返すかで動作が変わります。

| ハンドラーの戻り値 | 動作 |
| --- | --- |
| `Ok(Turn(state, Err(AuthError)))` | 認証不成立などの業務上の拒否。Turnに渡した次の状態を保存し、再起動しない |
| `Err(Error)` | worker自体の失敗。再起動方針を適用する |

認証不成立を401へ変換するのは、呼び出し側のHTTP mapperの役目です。返信の`Err(AuthError)`だけでHTTPステータスが決まるわけではありません。

再起動時には初期化関数を呼び直し、新しい状態を作ります。処理中や待機中だったメッセージを再配送しません。状態の復元が必要なら、初期化関数でDBなどから読み込みます。

既定の再起動間隔は10ms、回数はSupervisor全体で10秒間に5回までです。上限を超えると、ほかの子も停止して`run`がErrorを返します。明示的な停止や親のキャンセルは再起動の理由になりません。

`TEMPORARY`の子が失敗すると、その子を停止して失敗を記録します。他の子は動き続けます。実行中の子や再起動待ちがなくなるとSupervisorを終了し、後片付け後の`run`は記録したErrorを返します。

## 所有と停止

`run`へSupervisorを渡すと登録が締め切られます。`Control`は停止と監視のためのハンドルです。Controlを破棄しても、所有されているSupervisorは動き続けます。Supervisorや実行中の`run`を破棄すると、子へ停止を要求します。

[scope](async.md)本体が終わって子の終了を待つ段階で、spawnした`run`がErrorを返すと、同じscopeの残りの子をキャンセルして終了を待ちます。HTTPサーバーも同じscopeでspawnしていれば、その対象です。たとえば最後の`TEMPORARY` workerの失敗は、HTTPの終了につながります。業務上の拒否とworkerの故障を分けて返してください。

`shutdown`の成功は、子と共通データの後片付け完了を表します。Errorには、後片付け完了後に返す記録済みの子の失敗と、期限切れによる未完了の両方があります。現行APIには専用の完了状態型がなく、Errorだけでは後片付けが完了したかを区別できません。

期限内に終わらない場合は終了を追跡する記録を保持します。HTTPやnative処理が共通データを保持している間は、後片付けは完了しません。その解放がSupervisorの終了待ちに依存すると、互いに待つ構成になります。利用者側でも参照と停止の順序を設計する必要があります。Rust連携で別のTokio runtimeを使う場合は、`run`を開始したruntimeを後片付けまで維持します。

Tokioの停止は協調的です。yieldしないCPU処理、blockingなnative呼び出し、重いDropは強制中断できません。長い計算は分割して`await actor.yield_now()`を挟むか、終了を管理できる別プロセスで実行してください。共通データを持たずに始まった外部のblocking jobなどは、このグループの終了保証に含まれません。

## 監視

`next_event`で起動、失敗、再起動、停止を受け取れます。イベントは既定256件、診断文は最大1024 UTF-8 bytesです。読み手が遅れた場合は`EventKind.LAGGED`と失われた件数が届きます。監視の遅れで子の処理を止めません。

監視を無期限に待たせたくない場合は`next_event_timeout(view(control), 5000)`を使います。`Err(WaitError)`のkindで`TIMEOUT`と`INVALID_TIMEOUT`を区別できます。`Ok(None)`は列の終了だけを表します。読み取りロック待ちも期限に含まれ、キャンセル後も未読イベントを受け取れます。期限は1..4,294,967,295msかつ時計が表現できる値にします。

taskの`STARTED`はfactory本体の実行前に届きます。接続確立などの準備完了を待つ場合は`task_with_ready`で登録し、初期化後に`mark_ready(view(signal))`を呼びます。監視側へ子の名前と世代番号付きの`EventKind.READY`が届きます。重複や古い世代の通知は受け付けません。actorの起動確認は既存の`ready`を使います。

[workerのサンプル](../test-nagi-code/application-examples/supervised-worker/README.md)で準備完了通知、panicからの再起動、期限付き監視、停止を確認できます。

Tokioを使うnative実装で、BEAMの隔離されたprocessやVMと同じ障害耐性はありません。VM、無停止のコード差し替え、分散配置、永続mailbox、実行中の子の追加・削除、one-for-all／rest-for-oneは未対応です。

[actorの書き方](actor.md) · [APIリファレンス](actor-reference.md) · [性能測定](actor-performance.md) · [実行できるサンプル](../test-nagi-code/library-examples/supervised-service/README.md)

旧[supervisor.nagi](../examples/supervisor.nagi)は固定workerの再起動試験です。

実装は[Supervisor runtime](../runtime/src/actor.rs)と[ライフサイクル管理](../runtime/src/actor/lifecycle.rs)にあります。[再起動のテスト](../runtime/src/actor/tests.rs)と[停止・解放のテスト](../runtime/src/actor/lifecycle_adversarial_tests.rs)が、業務エラー、子の失敗、待機期限、後片付けの各条件を確認します。
