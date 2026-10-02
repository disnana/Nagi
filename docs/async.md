# asyncとscope

タイマーやDBなどの処理を待つ関数は`async def`で定義し、`await`で結果を待ちます。待っている間は、ほかの非同期処理を進められます。

```nagi
async def main() -> Result[unit, Error]:
    await sleep(10)
    return ok(print("待ち終わりました"))
```

`sleep`の引数はミリ秒です。上のコードは待ち終わってからメッセージを表示します。失敗する可能性のある処理では、`try await db_open(...)`のように結果のエラーも扱います。

## 複数の処理を始める

`async with scope`の中で`spawn`すると、子の処理を始められます。scopeを出るときに、すべての子の終了を待ちます。

```nagi
async def main() -> Result[unit, Error]:
    async with scope:
        spawn sleep(10)
        spawn sleep(15)
    return ok(print("完了"))
```

子が`Result`のエラーを返したりpanicしたりすると、残りをキャンセルして終了を待ちます。`spawn`できるのは、`unit`か`Result[unit, Error]`を返す非同期処理です。scope内の`return`と、viewを子へ渡すことは未対応です。

親の処理そのものが破棄された場合や、scope本体がpanicした場合には、子へ停止を要求します。その場で全員の終了を待つ保証はありません。CPU処理の停止については[並行処理](concurrency.md)を参照してください。

## 関数を変数に入れて呼び出す

async関数も、関数名を変数へ代入して呼び出せます。次の例は`42`を表示します。

```nagi
async def answer(value: i64) -> i64:
    return value + 1

async def main():
    selected = answer
    print(await selected(41))
```

これは関数そのものの代入です。`pending = answer(41)`のように呼び出した戻り値を保存する形式は未対応です。呼び出しとawaitを合わせて書いてください。関数を引数や戻り値として受け渡す型の範囲は[型と推論](types.md#関数を値として渡す)にあります。
