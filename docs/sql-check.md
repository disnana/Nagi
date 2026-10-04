# SQLの事前検査

Nagi 0.1.10以降では、`check`にschemaを指定すると、SQLiteのSQL文字列の名前・返却列・bind数を実行前に検査できます。通常の`check`や`build`・`run`では自動で有効になりません。版ごとの変更は[変更履歴](../CHANGELOG.md)を参照してください。

## 使い方

`schema.sql`に、アプリで使うテーブルの定義を保存します。

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);
```

`app.nagi`には、返す行の型とSQLを書きます。

```nagi
class User:
    id: i64
    name: str

async def main() -> Result[unit, Error]:
    db = try await db_open(":memory:")
    try await db_exec(db, "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
    users = try await db_all[User](db, "SELECT id, name FROM users")
    for user in users:
        print(user.name)
    return ok(print("Done"))
```

```sh
nagic check app.nagi --sql-schema schema.sql --sql-dialect sqlite
```

`SELECT id, naem FROM users`なら存在しない列、`SELECT id FROM users`ならUserに必要なname列の不足として失敗します。診断はSQLを書いたNagi・Lowのファイルと行を示します。

プロジェクトの場合は`nagic check --project ./app --sql-schema ./app/schema.sql --sql-dialect sqlite`です。schemaの相対パスは、コマンドを実行したディレクトリが基準です。2つのSQLオプションは組で指定し、`check`だけで使います。nagi.tomlの設定やエディターからの自動検査には対応していません。

## 検査するもの

| 対象 | 確認する内容 |
|---|---|
| `db_all`・`db_query` | 単一のSQL、テーブル・列の名前、bind数、行classに必要な返却列 |
| `db_insert`・`db_update` | 単一の書き込みSQL、bind数、`RETURNING`の必要な列 |
| `db_write` | 行を返さない単一の書き込みSQL、bind数 |

対象は、組み込みDB関数として解決された呼び出しへ直接渡した文字列リテラルです。同名の自作関数は対象にしません。列順の変更、適切なalias、余分な返却列は許します。placeholder数はSQLiteの規則に従い、同じ`?1`の再利用はbindを増やしません。

手書きRustの`FromRow`本体は解析しません。この検査ではclassのフィールド名を必要な返却列として扱うため、SQLに適切なaliasを付けてください。通常の`check`・`build`とRustの行読み取り処理は変わりません。

変数やRustで作った動的SQLは実行時の検査に残ります。`db_exec`の複数文・schema変更も対象外です。アプリ内のCREATE TABLEを実行してschemaを推測することはしません。結果には検査したリテラルの件数と、実行時に残る呼び出しの件数・位置・理由を表示します。

## schemaと実行時の境界

指定したschemaを新しいメモリ内DBに作り、SQLをprepareして名前・列・bindの情報を取得します。検査対象のqueryは実行せず、アプリのDBへ接続せず、Cargoも起動しません。通常の`check`はSQL用のconnectionやworkerを作りません。生成するRustと、アプリでのDB処理は変わりません。

schemaには通常のCREATE TABLE・INDEX・VIEWを使います。任意のmigrationを実行する機能ではありません。ATTACH・DETACH、PRAGMA、外部DBやファイルへの書き込み、extension読み込み、TEMP・virtual table、trigger、transaction、CREATE TABLE AS SELECTは拒否します。関数は許可リストを使い、randomや日時関数、`DEFAULT CURRENT_TIMESTAMP`などは対象外です。

schemaは2 MiB・1024文、各SQLは256 KiBまでで、列数や式の深さなどにも上限があります。別worker processを5秒の期限で監視し、上限や期限を超えた入力は検査エラーになります。

1回の検査では、すべての対象SQLを指定した1つのschemaに照合します。複数のDbの接続先は推測しません。実際に配備するDBのschemaも一致させてください。

値の型・整数範囲・NULL可否、query結果、権限や実DBの状態までは保証しません。たとえば`SELECT 'oops' AS id, 'Nagi' AS name FROM users`は必要な列を返すので、この検査だけではidの型不一致を検出できません。行の読み取りと動的SQLは、引き続き実行時のResultで処理します。[SQLiteの制約](database.md#実装と制約)も参照してください。

## SQLiteを含めずにコンパイラをビルドする

検査エンジンは、既定のCargo feature `sql-check`に含まれます。ソースから外してビルドする場合は次を使います。

```sh
cargo build --release --locked -p nagic --no-default-features
```

このコンパイラでも通常の検査とビルドは使えますが、SQL検査オプションを指定すると、featureが必要というエラーになります。アプリ側のSQLite runtimeとは別の設定です。
