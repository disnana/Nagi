# SQLite

SQLを文字列で渡し、paramはnative型のままbindします。値をSQLへ連結しません。`User`等のclassからFromRowを生成し、rowのprimitive/String fieldを直接読みます。tuple、dict、ORM objectは作りません。

```python
async def get_user(db: Db, id: i64) -> Result[User?, Error]:
    return await db_query[User](db, "SELECT id, name, age FROM users WHERE id = ?1", id)
```

SQLite connectionは専用threadが所有します。HTTP executorからは容量64のjob channelとoneshot replyで呼び出します。closure/jobとreplyのallocationは残ります。queryをasyncと書けてもSQLite内部がasync I/Oになるわけではありません。

prepared statementはcacheします。列名を一度だけ列番号へ解決し、各rowは番号で取得します。16列以内の番号配列はinlineで保持し、毎queryの小さなVec allocationを避けます。TEXT/BLOBを返す場合はSQLite rowの寿命を越えるため所有化します。

0.1のqueryはi64 param一つ、allはparamなし、insert/updateはCRUD sample用のname/age形式です。汎用の可変長typed param、transaction API、DB pool、schemaのコンパイル時検査は未実装です。列名・SQL・列値の型の不整合は実行時Resultになります。

SQL literalはworkerへstatic参照で渡せます。動的SQLは所有Stringを作ります。HTTP timeoutでcallerがキャンセルされても、既に受け付けたDB jobが完了・commitする場合があります。cancel＝rollbackではありません。PostgreSQL binary protocolは設計段階です。
