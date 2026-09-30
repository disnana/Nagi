# asyncとscope

async関数はネイティブFutureへ生成します。async呼び出しを捨てず、awaitするかscope内でspawnします。

```python
async def main() -> Result[unit, Error]:
    async with scope:
        spawn sleep(10)
        spawn sleep(15)
    return ok(print("完了"))
```

scopeの正常出口では全子taskをjoinします。Resultエラー時には残りをcancelし、破棄完了をawaitします。子taskのpanicもJoinErrorで検出して残りをcancelします。0.1のspawnはunitまたはResult[unit,Error]のasync関数に限定しています。

scope内のreturnは拒否します。親Futureそのものが外側からdropされる場合や、scope本体がpanicする場合はJoinSetのDropがabortを要求します。その場で全子taskの終了をawaitする保証はなく、協調的キャンセルが実行されるまで時間がかかります。

borrowed viewはspawnへ渡せません。必要なデータを所有化してmoveします。任意の借用taskを安全にspawnする仕様は今後の課題です。
