# asyncとscope

## 待ち終わった結果を使いたい

Pythonの`await asyncio.sleep(0.01)`のように、Nagiも`await`で待ちます。Pythonのsleepは秒、Nagiの`sleep`はミリ秒です。`await`しただけで、別taskや別threadを作るわけではありません。

タイマーやDBなどの処理を待つ関数は`async def`で定義し、`await`で結果を待ちます。NagiはこれをRustのFutureへ変換し、Tokio上で実行します。待っている間は、ほかの非同期処理を進められます。同期のCPU処理を自動で別スレッドへ移す機能ではありません。

```nagi
async def main() -> Result[unit, Error]:
    await sleep(10)
    return ok(print("待ち終わりました"))
```

`sleep`の引数はミリ秒です。上のコードは待ち終わってからメッセージを表示します。失敗する可能性のある処理では、`try await sqlite.open(path, config)`のように結果のエラーも扱います。

よくある間違いは、`pending = sleep(10)`と戻り値を保存してから待つことです。現在はFutureの保存に対応していません。`await sleep(10)`と呼び出しを直接待ってください。

```nagi
async def main():
    pending = sleep(10)  # check rejects storing this Future
    await pending
```

この例ではFutureをローカル変数にできないため、後から`await`する形は使えません。上の有効な例のように`await sleep(10)`と書くと、待機後にメッセージが一度表示されます。

**一言でいうと：awaitで結果を待つ。** Resultも返す呼び出しの扱いは[エラー処理](error-handling.md)を参照してください。

## 待っている間に、別の処理も進めたい

Pythonでは関連するtaskを`asyncio.TaskGroup`へ登録できます。

```python
import asyncio

async def main():
    async with asyncio.TaskGroup() as group:
        group.create_task(asyncio.sleep(0.01))
        group.create_task(asyncio.sleep(0.015))
    print("完了")

asyncio.run(main())
```

Nagiでは`async with scope`の中で`spawn`します。次はそのまま実行できる例です。

```nagi
async def main() -> Result[unit, Error]:
    async with scope:
        spawn sleep(10)
        spawn sleep(15)
    return ok(print("完了"))
```

どちらの待ち時間も終わってから`完了`を一度表示します。子同士がどの順番で実行されるかは保証しません。`spawn`は子を実行対象にする操作で、書いた瞬間に子の本体が動く保証もありません。

二つの`await sleep(...)`を順に書くと、最初の待ち時間が終わってから次を待ち始めます。重ねて進めたい場合は上のようにspawnし、通常のscope終了で子の終了を待ちます。

**一言でいうと：spawnで並行に進め、scopeで寿命を管理する。** PythonのTaskGroupと取消・失敗の全動作が同じではありません。正確な制約は次の節と[並行処理](concurrency.md)にあります。

## 旧statement spawnとscopeのリファレンス

旧statement spawnではscope本体が終わると子の結果を確認し、子のErrやpanicで残りをキャンセルして終了を待ちます。子の失敗が本体へ割り込むことはありません。一方、Task bindingをawaitすると本体の実行中でもそのTaskの結果を受け取れます。受取failureを処理してもscope faultは残り、scope出口で全子の実joinを待ちます。旧statement形式でspawnできるのは、`unit`か`Result[unit, Error]`を返す非同期処理です。scope内の`return`と、viewを子へ渡すことは未対応です。

引数は`spawn`を書いた場所で評価し、できた値を子へ渡します。`spawn work(copy(part))`のようにviewから所有値を作ると、元のデータを親でも使い続けられます。配列をコピーしても中身にviewが残る場合は、子へ渡せません。

scopeを使う関数はResultを返します。独自のエラーclass・enumを使う場合は、Rust連携で`From<nagi_runtime::Error>`を明示的に実装してください。scope出口の失敗をその型へ変換できることはビルド時に確認します。旧statement spawnの子がResultを返す場合、そのエラー型はErrorです。Task bindingの内側業務Resultは、対応する任意のエラー型を使えます。

scope本体の`try`でErrを伝えて退出する場合は、子をキャンセルして終了を待ってから外側へErrを伝えます。親のFutureそのものが破棄された場合や、scope本体がpanicした場合には、子へ停止を要求します。同期のDropでは非同期の終了待ちができないため、その場で全員の終了が完了している保証はありません。取消要求は、受理済みのDB操作などの外部副作用を巻き戻すものでもありません。CPU処理の停止については[並行処理](concurrency.md)を参照してください。

### 並行な子の結果を受け取る（Nagi 0.1.11以降）

S1の`task = spawn work()`、`await task`、`std.task.discard(task)`とS2のSupervisor monitor移行はNagi 0.1.11で公開済みです。S2は既存Task APIでmonitorをawaitし、`Ok(inner)`を親bodyの`try`へ渡します。利用するcompilerが0.1.11以降であることを確認してください。[Task結果handle](task-handles.md)に、全Tの一回消費、正常出口でのawait/discard義務、scope外へのescape拒否とTaskFailure APIをまとめています。内側の業務Errは兄弟を止めず、受取faultを処理してもscope故障は残ります。旧statement spawnは`Result[unit, Error]` Errで兄弟を取消す動作を維持します。公開SQLite Pool/Txは別工程です。

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

実装は[scopeのruntime](../runtime/src/concurrent.rs)と[コード生成](../compiler/src/emit.rs)にあります。[scopeのテスト](../compiler/tests/scoped_tasks.rs)と[async関数値のテスト](../compiler/tests/async_value_types.rs)が対応する入力と拒否する入力を示します。[実ランタイムのテスト](../compiler/tests/scope_runtime_contract.rs)では、子のエラー・panicと本体の`try`失敗について、本体の継続と子の破棄完了をHigh・保存Low・手書きLowで確認します。
