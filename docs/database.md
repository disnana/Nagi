# SQLite

[目次](README.md) · [SQLite PoolとTransaction](sqlite-pool.md) · [SQLの事前検査](sql-check.md)

開発sourceの標準SQLite入口は `std.db.sqlite` です。SF05で旧 `Db` と `db_open/db_exec/db_all/db_query/db_insert/db_update/db_write` を廃止し、checkerで具体的な移行診断を返します。正式0.2.0は未リリースで、公開済み0.1.xのbinaryへ遡及適用しません。

`Pool` を明示 `Options` で開き、`Tx` が一つの接続を専有します。SQL構造は直接文字列literalから `sqlite.literal(...)` で作る opaque `Query`、実値は `Parameters` で渡します。文字列変数、連結、format、動的文字列からQueryを作る入口はありません。Query自体の保存・選択・関数返却はできます。

```nagi
import std.db.sqlite as sqlite

class User:
    id: i64
    name: str

async def find(tx: view[sqlite.Tx], id: i64) -> Result[User?, sqlite.Failure]:
    sql = sqlite.literal("SELECT id, name FROM users WHERE id = ?")
    values = sqlite.bind_i64(sqlite.parameters(), id)
    return await sqlite.query[User](tx, sql, values)
```

メモリDBの作成・保存・読込・終了を含む実行例は [sqlite_pool.nagi](../examples/sqlite_pool.nagi) と[実行手順](sqlite-pool.md#まず動かす)を参照してください。相対DBパスは実行時current working directoryを基準にします。PostgreSQLなどはtrusted Rust連携が必要です。

| 旧操作 | 移行 |
|---|---|
| `db_open` | `sqlite.options` と `sqlite.open`、明示的な `begin` |
| `db_all` / `db_query` | `sqlite.all[T]` / `sqlite.query[T]`、QueryとParameters |
| `db_exec` / `db_write` | 一文の `sqlite.exec`。複数文は明示Tx内で分ける |
| `db_insert` / `db_update` の RETURNING | 同一Tx内のexecとreadonly query。INSERTは `last_insert_rowid()`、UPDATEは実idをbindして再読込する |

新APIは匿名 `?` のみを受け付け、`exec` は返却列のない文、`query/all` はreadonlyで列を返す文です。DDL/bootstrapはrequestから分離したtrusted管理処理に置きます。request由来のSQL構造を受け取るfactoryは再exportしません。[CRUD](../examples/crud.nagi)、[inventory](../test-nagi-code/inventory.nagi)、[タスクAPI](../test-nagi-code/web-demo/tasks.nagi)は明示的なpublic policyのデモです。

行classは対応scalar field、そのOptionとowned wrapperを読み取ります。実データの値型、NULL、数値範囲はruntimeのResultで検査します。対応外の手書きFromRowはtrusted native adapterから利用し、標準Nagi APIのscalar行制約を迂回しません。`all` の配列は非Copy行を複製せず読み取り専用で反復できます。

通常checkはliteral境界・型・所有権を検査し、SQLエンジンを起動しません。schemaを明示したopt-in SQL検査はSQL構造と必要列をprepare-onlyで確認し、直接Parameters builder列のbind数も照合します。Query変数やParameters変数を静的に追跡できない場合はruntime検査として表示します。

DB操作は `sqlite.Failure` を返し、`Error` へ暗黙変換しません。HTTPデモはprimary Errorを明示的に投影し、FailureのOutcome・cleanup・retired情報をHTTPエラー型に自動追加しません。業務で必要なら独自error型へ保持してください。

保護データはreview済みadapterでGrantの実subject/targetをtenant/owner predicateにbindします。汎用queryへGrantを渡すだけのtenant保証はありません。容量を予約してから `Grant.submit` の同期gateで有効性を検査し、一回のexecution permitを発行します。gateを解放した後、同期callbackで実SQLite queueへenqueueし、replyをawaitします。permit発行より失効が先ならenqueueは0です。permit発行後の失効はenqueue前でも受理済み操作を取り消しません。HTTP取消も受理済みcommandのrollbackを保証しません。[保護DB境界](security.md#保護sqlite操作)を参照してください。

取得予算、authorizer、取消後のcleanup、Outcome、0ms acquire、actual close/joinの契約は[SQLite reference](sqlite-pool.md)にあります。SQLのErrをtransactionのrollbackや「変更なし」と解釈しないでください。
