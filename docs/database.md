# SQLite

[目次](README.md) · 前：[HTTPとHTML](http.md) · 関数の引数：[組み込み関数](builtins.md)

## 最初の読み書き

保存する型をclassで定義し、`db_open`でDBを開き、`db_exec`でテーブルを作ります。DBの操作は非同期で、失敗し得るため`try await`を使います。

次のコードを`database.nagi`として保存し、`nagic run database.nagi`で実行します。メモリ内のDBへ1件追加し、保存した名前を表示します。

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

行を返す関数の型引数にはclassを指定します。`db_all[i64]`や`db_query[str]`は使えず、`check`でエラーになります。1列だけ読む場合も、その列を持つclassを定義してください。

| 操作 | 書き方 | await後の戻り値 |
|---|---|---|
| テーブル作成など | `db_exec(db, sql)` | `Result[i64, Error]` |
| 全行読む | `db_all[User](db, sql)` | `Result[List[User], Error]` |
| 1行読む | `db_query[User](db, sql, id)` | `Result[User?, Error]` |
| 追加 | `db_insert[User](db, sql, name, age)` | `Result[User, Error]` |
| 更新 | `db_update[User](db, sql, id, name, age)` | `Result[User, Error]` |
| 削除など | `db_write(db, sql, id)` | `Result[i64, Error]` |

`db_exec`の行数は、その呼び出しで実行したSQL全体の変更件数です。テーブル作成やSELECTだけなら0を返します。複数の文の変更を合計し、トリガーや外部キー制約による変更もSQLiteの集計に従って含めます。

現時点ではbind引数の形が固定です。query / writeはi64が1つ、allはなし、insertはstrとi32、updateはi64・str・i32です。name / ageという列名は必須ではなく、同じ引数型を持つ別のテーブルにも使えます。

## APIから読む

SQLの値は引数として渡します。文字列を連結してSQLを組み立てる必要はありません。読み込んだ行は、指定したclassのフィールドへ入ります。

次は上のUser型を使ったhandlerの断片です。

```nagi
async def get_user(db: Db, id: i64) -> Result[User?, Error]:
    return await db_query[User](db, "SELECT id, name, age FROM users WHERE id = ?1", id)
```

classの`List`を`for`で走査する場合、現在はCopy classのみ対応します。上のUserは所有strを持つため、そのまま`for user in users`とは書けません。HTTP handlerで`return await db_all[User](...)`と返してJSON配列にすることはできます。完成例は[CRUD API](../examples/crud.nagi)や[タスク管理](../test-nagi-code/web-demo/tasks.nagi)を参照してください。

## 実装と制約

SQLiteへの操作は専用のスレッドで順に実行します。待ち行列は64件までです。Nagi側は非同期に結果を待ちますが、SQLiteそのものの読み書きはそのスレッドで行います。

SQLの準備結果と列名の解決結果を再利用します。文字列やバイト列を返すときは、行の処理が終わったあとも保持できるよう、所有する値を作ります。

任意個数のSQL引数、トランザクション専用API、接続プール、ビルド時のスキーマ検査は未対応です。SQLや列の名前・型に問題がある場合は、実行時にResultのエラーになります。

HTTPの待機期限が切れても、すでに受け付けたDB操作が完了し、書き込みが保存される場合があります。呼び出し元のキャンセルで、書き込みが取り消される保証はありません。
