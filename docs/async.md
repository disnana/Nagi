# asyncとscope

タイマーやDBなどの処理を待つ関数は`async def`で定義し、`await`で結果を待ちます。NagiはこれをRustのFutureへ変換し、Tokio上で実行します。待っている間は、ほかの非同期処理を進められます。同期のCPU処理を自動で別スレッドへ移す機能ではありません。

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

scope本体が終わると、子の結果を確認します。子が`Result`のエラーを返したりpanicしたりすると、残りをキャンセルして終了を待ちます。scope本体の実行中に子の失敗で割り込む動作はありません。`spawn`できるのは、`unit`か`Result[unit, Error]`を返す非同期処理です。scope内の`return`と、viewを子へ渡すことは未対応です。

引数は`spawn`を書いた場所で評価し、できた値を子へ渡します。`spawn work(copy(part))`のようにviewから所有値を作ると、元のデータを親でも使い続けられます。配列をコピーしても中身にviewが残る場合は、子へ渡せません。

scopeを使う関数はResultを返します。独自のエラーclass・enumを使う場合は、Rust連携で`From<nagi_runtime::Error>`を明示的に実装してください。子の失敗をその型へ変換できることはビルド時に確認します。spawnする子のエラー型は引き続きErrorです。

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

async関数を入れた変数に、別のasync関数を再代入することはできません。別の変数を使うか、ifの各分岐で呼び出してください。同じ関数を入れ直すことや、同期関数を入れた変数の差し替えはできます。

実装は[scopeのruntime](../runtime/src/concurrent.rs)と[コード生成](../compiler/src/emit.rs)にあります。[scopeのテスト](../compiler/tests/scoped_tasks.rs)と[async関数値のテスト](../compiler/tests/async_value_types.rs)が対応する入力と拒否する入力を示します。
