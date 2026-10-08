# SQLite

[目次](README.md) · 前：[HTTPとHTML](http.md) · 関数の引数：[組み込み関数](builtins.md)

## 最初の読み書き

このページは従来の標準DB API `db_*` を説明します。保存する型をclassで定義し、`db_open`でDBを開き、`db_exec`でテーブルを作ります。失敗し得る非同期操作なので、`try await`で結果を扱います。PostgreSQLなど、別のDBには現在Rust連携が必要です。

> 開発sourceには別の `std.db.sqlite` Pool/Transaction APIがありますが、Nagi 0.1.11には未収録です。新APIの使用法は[SQLite PoolとTransaction](sqlite-pool.md)、この旧APIの制約は以下を参照してください。

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

標準の行読み取りは、`i8`・`i16`・`i32`・`i64`・`u8`・`u16`・`u32`・`f32`・`f64`・`bool`・`str`・`bytes`のフィールドに対応します。各型を`T?`にするとSQLのNULLを受け取れます。boolやfloat、bytesのnullable対応はNagi 0.1.9以降です。[設定APIの例](../test-nagi-code/application-examples/device-settings/README.md)で確認できます。

Nagi 0.1.10以降では、これらの型を`owned[...]`で包んだフィールドにも行の読み取り実装を自動生成します。`owned[str]`や`owned[i64?]`に手書きの`FromRow`は不要です。以前の回避策として追加していた場合は、重複する実装を削除してください。対応外のフィールドは、Rust側の読み取り実装や型変換を使います。

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

`db_all[User]`で得た配列は、`for user in users`で読み取り専用に走査できます。Userが持つ文字列を反復のためにコピーしません。借用の制約は[所有権](ownership.md)を参照してください。HTTP handlerで`return await db_all[User](...)`と返してJSON配列にすることはできます。完成例は[CRUD API](../examples/crud.nagi)や[タスク管理](../test-nagi-code/web-demo/tasks.nagi)を参照してください。

## 従来APIの実装と制約

この節は従来の `Db` / `db_*` APIだけを説明します。Nagiは型付き呼び出しとclassへの行変換を提供し、SQLの実行・保存・制約の検査はrusqliteとSQLiteが担います。SQLiteはruntimeに同梱されます。

このAPIでは開いたDbごとに専用スレッドで操作を順に実行します。待ち行列は64件までで、満杯なら送信側が待ちます。Nagi側は結果を非同期に待ちますが、SQLiteの読み書き自体は同期処理です。Dbの最後の所有者が解放される際はworkerの終了を待つため、即時に戻る保証はありません。新しいPool APIのworkerとqueue条件は[別ページ](sqlite-pool.md)にあります。

このAPIはSQLの準備結果をキャッシュします。列名は呼び出しごとに解決し、`db_all`では同じ結果の各行にその列位置を使います。文字列やバイト列を返すときは、行の処理が終わったあとも保持できるよう、所有する値を作ります。

通常の`check`はNagiの引数型と、行を返す型がclassであることを検査します。SQL文字列の構文、schema、列名、bind数、SQLのNULLとclassの対応は検査しません。

Nagi 0.1.10以降では、[SQLの事前検査](sql-check.md)を明示的に指定すると、SQLiteの文字列リテラルの構文・名前・必要な返却列・bind数を確認できます。動的SQL、従来APIの`db_exec`、実データの型・NULL・値の範囲は対象外で、対応するフィールド型では実行時のResultで扱います。未対応の行フィールドなど、Rustの変換要件を満たさない型はbuildで失敗する場合もあります。開発sourceの新APIでは`query` / `all` / `exec`のliteral SQLも検査し、Parametersのbind数と値型は未検査としてruntimeに残します。詳細は[SQL事前検査](sql-check.md)を参照してください。

従来の`db_*` APIでは任意個数のSQL引数、複数呼び出しを専有するtransaction、connection poolは未対応です。SQLでBEGIN／COMMITを書いても複数呼び出しを専有しないため、共有Dbの他の操作が間へ入る可能性があります。開発sourceの新APIには明示TxとPoolがありますが、0.1.11には含まれません。

HTTPの待機期限が切れても、すでに受け付けたDB操作が完了し、書き込みが保存される場合があります。呼び出し元のキャンセルで、書き込みが取り消される保証はありません。

従来APIの実装は[DB runtime](../runtime/src/database.rs)と[classの行変換生成](../compiler/src/emit.rs)、確認用のコードは[CRUDサンプル](../examples/crud.nagi)と[DBのテスト](../runtime/src/database.rs)にあります。新APIの使い方は[SQLite PoolとTransaction](sqlite-pool.md)を、SQLite／PostgreSQLの別型と共通操作規則の設計は[ライブラリ設計案](library-design.md)を参照してください。
