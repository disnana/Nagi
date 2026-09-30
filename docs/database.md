# SQLite

[目次](README.md) · 前：[HTTPとHTML](http.md) · 関数の引数：[よく使う関数](builtins.md)

## 最初の読み書き

保存する型をclassで定義し、`db_open`でDBを開き、`db_exec`でテーブルを作ります。DBの操作は非同期で、失敗し得るため`try await`を使います。

次は完全なコードです。メモリ内に1件追加して読み、`Nagi`と表示します。

```nagi
class User:
    id: i64
    name: str
    age: i32

async def main() -> Result[unit, Error]:
    db = try await db_open(":memory:")
    try await db_exec(db, "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER NOT NULL)")
    user = try await db_insert[User](db, "INSERT INTO users(name, age) VALUES (?1, ?2) RETURNING id, name, age", "Nagi", 18)
    print(user.name)
    return ok(print("保存できた"))
```

ファイルに保存するなら`db_open("app.sqlite")`にします。相対DBパスはプログラムを実行したディレクトリが基準です。ソースを基準にするimportやinclude_textとは異なります。

`db_insert[User]`の`[User]`は返したい型です。`RETURNING`の列名と型をclassに合わせます。値は`?1`などへbindし、SQL文字列へ連結しないでください。

| 操作 | 書き方 | await後の戻り値 |
|---|---|---|
| テーブル作成など | `db_exec(db, sql)` | `Result[i64, Error]` |
| 全行読む | `db_all[User](db, sql)` | `Result[List[User], Error]` |
| 1行読む | `db_query[User](db, sql, id)` | `Result[User?, Error]` |
| 追加 | `db_insert[User](db, sql, name, age)` | `Result[User, Error]` |
| 更新 | `db_update[User](db, sql, id, name, age)` | `Result[User, Error]` |
| 削除など | `db_write(db, sql, id)` | `Result[i64, Error]` |

現時点ではbind引数の形が固定です。query / writeはi64が1つ、allはなし、insertはstrとi32、updateはi64・str・i32です。name / ageという列名は必須ではなく、同じ引数型を持つ別のテーブルにも使えます。

## APIから読む

SQLを文字列で渡し、paramはnative型のままbindします。値をSQLへ連結しません。`User`等のclassからFromRowを生成し、rowのprimitive/String fieldを直接読みます。tuple、dict、ORM objectは作りません。

次は上のUser型を使ったhandlerの断片です。

```nagi
async def get_user(db: Db, id: i64) -> Result[User?, Error]:
    return await db_query[User](db, "SELECT id, name, age FROM users WHERE id = ?1", id)
```

classの`List`を`for`で走査する場合、現在はCopy classのみ対応します。上のUserは所有strを持つため、そのまま`for user in users`とは書けません。HTTP handlerで`return await db_all[User](...)`と返してJSON配列にすることはできます。完成例は[CRUD API](../examples/crud.nagi)や[タスク管理](../test-nagi-code/web-demo/tasks.nagi)を参照してください。

## 実装と制約

SQLite connectionは専用threadが所有します。HTTP executorからは容量64のjob channelとoneshot replyで呼び出します。closure/jobとreplyのallocationは残ります。queryをasyncと書けてもSQLite内部がasync I/Oになるわけではありません。

prepared statementはcacheします。列名を一度だけ列番号へ解決し、各rowは番号で取得します。16列以内の番号配列はinlineで保持し、毎queryの小さなVec allocationを避けます。TEXT/BLOBを返す場合はSQLite rowの寿命を越えるため所有化します。

汎用の可変長typed param、transaction API、DB pool、schemaのコンパイル時検査は未実装です。列名・SQL・列値の型の不整合は実行時Resultになります。

SQL literalはworkerへstatic参照で渡せます。動的SQLは所有Stringを作ります。HTTP timeoutでcallerがキャンセルされても、既に受け付けたDB jobが完了・commitする場合があります。cancel＝rollbackではありません。PostgreSQL binary protocolは設計段階です。
