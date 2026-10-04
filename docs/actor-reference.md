# std.actor リファレンス

```nagi
import std.actor as actor
```

[書き方](actor.md) · [再起動と停止](supervisor.md)

## 型

| 型 | 内容 |
| --- | --- |
| `Supervisor[C]` | 共通データCと子の構成を所有する |
| `TaskReady` | taskの一世代だけで使う準備完了通知 |
| `Control` | 停止とイベント監視のハンドル |
| `Actor[M, R, E]` | メッセージM、返信R、業務エラーEを持つactorのハンドル |
| `Turn[S, R, E]` | 次の状態Sと`Result[R, E]` |
| `Options` / `ActorOptions` | グループ / actorの設定 |
| `RestartPolicy` / `CallKind` / `EventKind` | 比較できる定数型 |
| `WaitError` / `WaitKind` | イベント待ちの失敗。`TIMEOUT`、`INVALID_TIMEOUT` |
| `CallError` / `Event` | 呼び出しの失敗 / ライフサイクルイベント |

Cは初期化関数へ`shared[C]`として渡します。Sはactorだけが所有します。M・R・Eは容量を数えられる所有された値が必要です。class・enum・List・Option・Resultを使えますが、Map・view・shared・opaque resourceは含められません。

## 登録と実行

| 呼び出し | 戻り値 |
| --- | --- |
| `supervisor[C](context, options)` | `Supervisor[C]` |
| `control(view(group))` | `Control` |
| `clone_control(view(control))` | 独立したイベント読み取り位置を持つ`Control` |
| `register[S, M, R, E](view(group), name, factory, handler, options)` | `Result[Actor[M, R, E], Error]` |
| `task(view(group), name, factory, policy)` | `Result[unit, Error]` |
| `task_with_ready(view(group), name, factory, policy)` | `Result[unit, Error]` |
| `mark_ready(view(signal))` | `Result[unit, Error]` |
| `turn[S, R, E](next_state, reply)` | `Turn[S, R, E]` |
| `await run(group)` | `Result[unit, Error]`。groupを消費する |
| `await shutdown(view(control))` | `Result[unit, Error]` |
| `await next_event(view(control))` | `Result[Option[Event], Error]` |
| `await next_event_timeout(view(control), timeout_ms)` | `Result[Option[Event], WaitError]` |
| `await yield_now()` | `unit` |

factoryとhandlerは名前付きasync関数です。actorのfactoryは`shared[C] -> Result[S, Error]`、handlerは`(S, M) -> Result[Turn[S, R, E], Error]`。taskのfactoryは`shared[C] -> Result[unit, Error]`です。登録名は重複しない1..128 UTF-8 bytesにします。

`task_with_ready`のfactoryは`(shared[C], TaskReady) -> Result[unit, Error]`です。初期化後に`mark_ready`を呼ぶと、その世代の`READY`を一度だけ発行します。重複、終了・キャンセル・再起動済みの世代、停止中の通知は`Error`になります。通知用tokenはSupervisorや共通データの寿命を延ばしません。既存`task`の引数と`STARTED`の意味は変わりません。

## 呼び出し

| 呼び出し | 戻り値 |
| --- | --- |
| `clone_actor(view(handle))` | 同じactorへのハンドル |
| `await ready(view(handle), timeout_ms)` | `Result[unit, CallError]` |
| `await call(view(handle), message, mailbox_ms, reply_ms)` | `Result[Result[R, E], CallError]` |

readyの0msは現在の状態だけを確認します。callのmailboxは0msなら即時の受け入れを試みます。replyは正の値が必要です。返信期限は受理を起点とし、期限内に生成された返信は呼び出し側の再開が遅くても受け取れます。受け入れ後のタイムアウトは更新の取り消しを意味しません。これらの借用するasync呼び出しをspawnする場合は、ハンドルを所有する名前付きasync関数で包みます。

`CallError.kind`は`NOT_READY`、`MAILBOX_FULL`、`MAILBOX_TIMEOUT`、`MESSAGE_TOO_LARGE`、`REPLY_TOO_LARGE`、`STOPPED`、`RESTARTING`、`REPLY_LOST`、`REPLY_TIMEOUT`。`.message`は`view[str]`です。

## 設定

| 呼び出し | 内容 / 既定値 |
| --- | --- |
| `default_options()` | 子64、イベント256、10秒間に再起動5回、停止期限10秒 |
| `options(children, events, restarts, window_ms, shutdown_ms)` | `Result[Options, Error]` |
| `restart_delay(options, milliseconds)` | `Result[Options, Error]`。既定10ms |
| `default_actor_options()` | 64件、入力1MiB、返信1MiB、起動期限5秒、TRANSIENT |
| `actor_options(messages, message_bytes, reply_bytes, startup_ms, policy)` | `Result[ActorOptions, Error]` |

容量には、受け入れ済みで処理中のメッセージも含みます。返信が大きすぎる場合は`REPLY_TOO_LARGE`を返しますが、次の状態は保存します。計量はinlineの値と保持しているバッファ容量を数え、深さ64・訪問65,536までです。拒否された値の破棄や、呼び出し側・C・Sのメモリは別です。

## イベント

`Event.kind`は`STARTING`、`STARTED`、`READY`、`FAILED`、`PANICKED`、`RESTART_SCHEDULED`、`STOPPED`、`INTENSITY_EXCEEDED`、`SHUTDOWN`、`LAGGED`です。

`.child_id`、`.generation`、`.lost_events`はi64、`.truncated`はbool、`.child_name`と`.message`は`view[str]`です。読み取り位置はControlごとに独立します。同じControlでの読み取りは直列化され、終了後は残ったイベントを読み切ってNoneになります。

`next_event_timeout`はControlの読み取りロック待ちも期限に含めます。期限は1..4,294,967,295msで、時計が表現できる値にします。0・負値・上限超過は`WaitKind.INVALID_TIMEOUT`、期限切れは`WaitKind.TIMEOUT`、`Ok(None)`は列の終了です。タイムアウトやキャンセルでイベントを消費したり、グループを停止したりしません。`WaitError.message`は`view[str]`です。期限は協調的に扱われ、blockingなnative処理を強制中断しません。
