# Resultとpanic

通常の失敗は`Result[T, Error]`です。例外だけに頼らず、戻り値に失敗を示します。

```python
def read_id(text: str) -> Result[i64, Error]:
    id = try parse_i64(text)
    return ok(id)
```

`try`はErrを呼び出し元へ返します。Resultを返す関数で使用します。直接捨てたResult、awaitしていないFutureはcheckerが拒否します。代入したResultの全経路での処理までを検査する機能は未完成です。matchによる分岐も未実装です。

recover可能なJSON/DB/validationエラーとpanicを区別します。scopeでは子taskのpanicを検出し、Supervisorではworkerのpanicを再起動対象にします。メモリ破壊・process abortの回復機構ではありません。

診断はsource path、行、該当ソース、理由とhelpを表示します。現時点のsource spanは行中心です。Lowの統合やRust backendの診断をHighの厳密な列位置へ戻すsource mapは未実装です。
