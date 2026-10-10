# actor

## 外から依頼を送り、状態を順番に更新したい

Pythonの`asyncio.Queue`から専用workerが仕事を受け取り、workerがカウンターを更新する構成を考えてください。actorも外側からメッセージで依頼し、自分の状態を一件ずつ更新します。外側からactorの状態を直接書き換えるAPIではありません。

Nagi 0.1.8から使える`std.actor`では、初期化とhandlerをasync関数で定義します。メッセージ・状態・返信の型をNagiで指定し、実行と通知にはTokioを使います。

次はhandlerの定義を示す断片です。登録と起動を含む完全なプログラムへのリンクは下にあります。

```nagi
import std.actor as actor

class Counter:
    total: i64

async def add(state: Counter, amount: i64) -> Result[actor.Turn[Counter, i64, Error], Error]:
    total = state.total + amount
    next_state = Counter(total=total)
    return ok(actor.turn[Counter, i64, Error](next_state, ok(total)))
```

たとえば`total=3`の状態へ`amount=2`を渡すと、次の状態の`total`も成功返信も`5`になります。actorとして登録すると、このhandlerがメッセージごとに順番に呼ばれます。

よくある間違いは、入力の拒否をhandler自身の`Err(Error)`で返すことです。それはworkerの故障として再起動方針の対象になります。想定した拒否は`ok(actor.turn(..., fail(problem)))`のようにTurnの返信へ入れ、次の状態を返してください。

これは文法上のエラーではなく、エラーを返す場所による動作の違いです。次の登録済みhandlerが負の金額を通常拒否したい場合、左はworkerの失敗として扱われます。

```nagi
async def add(state: Counter, amount: i64) -> Result[actor.Turn[Counter, i64, Error], Error]:
    if amount < 0:
        return error("amount must be non-negative")  # supervisor restart policy applies
    next_state = Counter(total=state.total + amount)
    return ok(actor.turn[Counter, i64, Error](next_state, ok(next_state.total)))
```

拒否を呼び出し元への返信にしたいなら、Turnの外側を`ok(...)`にします。

```nagi
async def add(state: Counter, amount: i64) -> Result[actor.Turn[Counter, i64, Error], Error]:
    if amount < 0:
        return ok(actor.turn[Counter, i64, Error](state, error("amount must be non-negative")))
    next_state = Counter(total=state.total + amount)
    return ok(actor.turn[Counter, i64, Error](next_state, ok(next_state.total)))
```

この場合、負の金額は返信の`Err`になり状態を変えず、workerの再起動にはなりません。

**一言でいうと：状態の更新はmessageで頼み、業務上の拒否は返信で返す。** 正確なAPIは[actorリファレンス](actor-reference.md)、失敗の区別は次の節にあります。

`Turn`は次の状態と返信をまとめた値です。状態を毎回コピーする必要はありません。登録・起動・呼び出し・停止までのコードは、[実行できるサンプル](../test-nagi-code/library-examples/supervised-service/README.md)にあります。

## 失敗を分ける

| 返す場所 | 意味 |
| --- | --- |
| `Turn`の返信が`Err(E)` | 想定した失敗。次の状態を保存し、処理を続ける |
| ハンドラー自身が`Err(Error)`、またはpanic | workerの故障。Supervisorが再起動方針を適用する |
| `call`の外側が`Err(CallError)` | 未起動、満杯、停止、返信のタイムアウトなど。`CallError.kind`で区別する |

返信には独自のclassやenumを使えます。`call`の型は`Result[Result[R, E], CallError]`です。業務上のエラーと、呼び出しのエラーを別々に扱います。

## 容量とタイムアウト

既定では一つのactorにつき、処理中を含む64件と、受け入れた入力の容量1MiBまでです。数値だけのListは要素を巡回せずに容量を確認します。文字列や入れ子の値はバッファ容量も数えます。状態や呼び出し側の待機中の値は別なので、プロセス全体のメモリ上限ではありません。

`call`の待ち時間は、受け入れ前の`mailbox_ms`と受け入れ後の`reply_ms`に分かれます。返信が時間切れになっても、受け入れ済みの更新は続く場合があります。二重更新を防ぐキーや結果確認を用意してから再試行してください。

メッセージ・返信にはMap、view、shared、native resourceを使えません。共通データとactorの状態には、必要な所有権とRustの`Send`／`Sync`条件を満たすsqlite.Poolなども使えます。任意のRust資源型をNagiへ登録する公開APIは未実装です。

条件を満たすshared messageを将来許す方向は採用していますが、現行の受理範囲ではありません。thread安全性・容量計算・保持する資源・返信への流出を確認する詳細設計が必要です。actor自身の状態と、明示的に共有したDBなどの外部資源の状態は区別します。設計の境界は[DESIGN](../DESIGN.md)を参照してください。

[Supervisorの再起動と停止](supervisor.md) · [APIリファレンス](actor-reference.md) · [性能測定](actor-performance.md)

旧[actor.nagi](../examples/actor.nagi)は固定カウンターの試験です。汎用APIの性能とは分けて扱います。

実装は[actor runtime](../runtime/src/actor.rs)、型と呼び出しの検査は[compilerのテスト](../compiler/tests/actor_stdlib.rs)、状態・再起動・容量の確認は[runtimeのテスト](../runtime/src/actor/tests.rs)にあります。
