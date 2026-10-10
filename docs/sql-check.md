# SQLの事前検査

開発sourceの `std.db.sqlite` は、通常checkで直接literalから作る `Query` を必須にします。SQL構文・schemaの検査は明示オプションで有効にする別段階です。SF05は未リリースで、正式0.2.0や公開済み0.1.xの機能として扱いません。[SQLite](sqlite-pool.md)と[移行](migration-0.2.0.md)を参照してください。

## 使い方

`schema.sql` に配備するschemaのDDL snapshotを保存します。

```sql
CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
```

`app.nagi` の直接Query constructorは、schemaの名前・返却列・bind数を検査できます。

```nagi
import std.db.sqlite as sqlite
class User:
    id: i64
    name: str
async def find(tx: view[sqlite.Tx], id: i64) -> Result[User?, sqlite.Failure]:
    return await sqlite.query[User](tx, sqlite.literal("SELECT id, name FROM users WHERE id = ?"), sqlite.bind_i64(sqlite.parameters(), id))
```

```sh
nagic check app.nagi --sql-schema schema.sql --sql-dialect sqlite
```

`naem` は未定義列、`SELECT id` はUserのname不足として拒否します。診断はSQLを書いた元Nagi/Low moduleの行を示します。projectでは `nagic check --project ./app --sql-schema ./app/schema.sql --sql-dialect sqlite` を使います。schemaの相対pathは実行時current working directoryを基準にします。二つのSQLオプションは組で指定し、checkだけに使えます。nagi.tomlやeditorの自動有効化はありません。

## 検査するもの

| 対象 | 確認する内容 |
|---|---|
| `sqlite.query/all` の直接literal Query | 単一readonly文、table/column名、必要row field |
| `sqlite.exec` の直接literal Query | 一文・返却列なし・schema参照。DDLもprepare-onlyで実行しない |
| 直接 `parameters()` と `bind_*` のbuilder列 | SQLの匿名 `?` 個数との一致 |
| Query/Parametersが変数や関数経由で構造不明 | 当該siteをruntime検査として表示。static検査成功へ数えない |

canonical `stdlib:std.db.sqlite` として解決された呼出しだけが対象です。同名ユーザー関数をSQLと推測しません。列順・alias・余分な列は許します。numbered `?1` とnamed placeholderは標準APIの制約で拒否します。SQL parserをcheckerへ複製せず、opt-in engineとruntimeがSQLiteのauthorizer・prepare metadataでshapeを検査します。

Query変数はliteralから作った値として通常checkを通りますが、このcollectorはdataflowを追ってSQLを推測しません。直接constructorでもParameters変数なら `bind unchecked` です。実値型・NULL・数値範囲は常にruntimeに残します。例えば `'oops' AS id` は必要列を返しても、i64のdecode成功を保証しません。手書きnative FromRow本体は解析しません。

DDL/bootstrapはtrusted管理処理です。queryを実行してschemaを推測せず、execのCREATE TABLEで検査schemaを変更しません。request用動的文字列factoryはありません。旧Db/db_*はSQL検査に入る前に具体的migration診断で拒否します。

## 安全性と保証の限界

検査用SQLiteは別workerのmemory DBです。通常CREATE TABLE・INDEX・VIEWを使うDDL snapshotを読み込みます。任意migration、ATTACH/DETACH、PRAGMA、外部file操作、extension、TEMP・virtual table・trigger・transaction・CREATE TABLE AS SELECTは拒否します。関数allowlistを使い、random・日時関数・DEFAULT CURRENT_TIMESTAMPは対象外です。

schemaは2 MiB・1024文、各SQLは256 KiBまでで、列数や式の深さにも上限があります。別worker processを5秒の期限で監視し、上限・期限違反は検査Errです。全siteを明示した一つのschemaへ照合し、複数Poolの配備先や権限・tenant predicate・実DBの状態を保証しません。

## SQLiteを含めずにコンパイラをビルドする

opt-in engineは既定Cargo feature `sql-check` に含まれます。

```sh
cargo build --release --locked -p nagic --no-default-features
```

engineなしでもliteral Queryの通常checkとbuildは使えます。SQL検査オプションはfeatureが必要というErrになります。アプリ側SQLite runtimeとは別設定です。
