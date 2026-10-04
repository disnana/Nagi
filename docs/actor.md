# actor

actorは、メッセージを一件ずつ処理し、自分の状態を更新します。Nagi 0.1.8から使える`std.actor`では、初期化とhandlerをasync関数で定義します。メッセージ・状態・返信の型をNagiで指定し、実行と通知にはTokioを使います。

```nagi
import std.actor as actor

class Counter:
    total: i64

async def add(state: Counter, amount: i64) -> Result[actor.Turn[Counter, i64, Error], Error]:
    total = state.total + amount
    next_state = Counter(total=total)
    return ok(actor.turn[Counter, i64, Error](next_state, ok(total)))
```

`Turn`は次の状態と返信をまとめた値です。状態を毎回コピーする必要はありません。登録・起動・呼び出し・停止までのコードは、[実行できるサンプル](../test-nagi-code/library-examples/supervised-service/README.md)にあります。

## 失敗を分ける

| 返す場所 | 意味 |
| --- | --- |
| `Turn`の返信が`Err(E)` | 想定した失敗。次の状態を保存し、処理を続ける |
| ハンドラー自身が`Err(Error)` | actorの失敗。Supervisorが再起動方針を適用する |
| `call`の外側が`Err(CallError)` | 未起動、満杯、停止、返信のタイムアウトなど。`CallError.kind`で区別する |

返信には独自のclassやenumを使えます。`call`の型は`Result[Result[R, E], CallError]`です。業務上のエラーと、呼び出しのエラーを別々に扱います。

## 容量とタイムアウト

既定では一つのactorにつき、処理中を含む64件と、受け入れた入力の容量1MiBまでです。数値だけのListは要素を巡回せずに容量を確認します。文字列や入れ子の値はバッファ容量も数えます。状態や呼び出し側の待機中の値は別なので、プロセス全体のメモリ上限ではありません。

`call`の待ち時間は、受け入れ前の`mailbox_ms`と受け入れ後の`reply_ms`に分かれます。返信が時間切れになっても、受け入れ済みの更新は続く場合があります。二重更新を防ぐキーや結果確認を用意してから再試行してください。

メッセージ・返信にはMap、view、shared、native resourceを使えません。共通データとactorの状態には、必要な所有権とRustの`Send`／`Sync`条件を満たすDbなども使えます。任意のRust資源型をNagiへ登録する公開APIは未実装です。

[Supervisorの再起動と停止](supervisor.md) · [APIリファレンス](actor-reference.md) · [性能測定](actor-performance.md)

旧[actor.nagi](../examples/actor.nagi)は固定カウンターの試験です。汎用APIの性能とは分けて扱います。

実装は[actor runtime](../runtime/src/actor.rs)、型と呼び出しの検査は[compilerのテスト](../compiler/tests/actor_stdlib.rs)、状態・再起動・容量の確認は[runtimeのテスト](../runtime/src/actor/tests.rs)にあります。
