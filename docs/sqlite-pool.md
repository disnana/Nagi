# SQLite PoolとTransaction

> `std.db.sqlite` は現在の開発ソースにある未リリースAPIです。Nagi 0.1.11には含まれていません。SF05で旧Db/db_*を廃止するため、開発sourceへ移行する際は[移行](migration-0.2.0.md)を参照してください。

[Docs目次](README.md) · [SQLiteと移行](database.md) · [SQLの事前検査](sql-check.md)

Rustからcompilerの標準module/resource/operation metadata enumを網羅matchする組込み利用者は、新しいQuery/literal variantへの対応が必要です。

## 何ができるか

`Pool` は複数のSQLite接続を管理し、`Tx` は一つの接続を専有します。SQL構造はopaque `Query`、実値はtyped owned `Parameters`で渡します。旧Db/db_*、runtime public Db/Sql、動的文字列factoryは削除し、旧Nagi入口はSF05 migration診断にします。旧APIとの併走fallbackはありません。

## まず動かす

### 必要なもの

- Rust toolchain（`rustc` と `cargo`）とRust crateをビルドできるC toolchain。
- このページと`examples/sqlite_pool.nagi`、未リリースの`std.db.sqlite` APIを含むNagiのソースcheckout（取得したソースの作業用フォルダー）。手順はrepository rootで実行します。
- SQLiteのコマンドラインツールは不要です。SQLiteはNagi runtimeに同梱されます。

ソースの取得方法は[ソースからcompilerをbuildする手順](getting-started.md#ソースからビルドする場合)を参照してください。`main`にまだこのsampleとAPIがない場合は、それらを含む開発branchのcheckoutを使います。配布版0.1.11のbinaryだけでは、この手順は実行できません。

`cargo build` がnative dependencyで止まる場合は、Rust toolchainに加えてOS向けC compiler/linkerが使えることを確認してください。`nagic` が見つからない場合は、下のrelease binaryを直接実行するか `target/release` をPATHへ追加します。

### コンパイラとサンプルを実行する

repository root（`Cargo.toml` があるdirectory）で、compilerをbuildし、サンプルをcheckしてから実行します。

```sh
cargo build --release --locked -p nagic
./target/release/nagic check examples/sqlite_pool.nagi
./target/release/nagic run examples/sqlite_pool.nagi
```

`check` が成功すると診断なしで終了し、`run` は次を標準出力へ表示します。

```text
7
closed
```

Windowsでは同じrepository rootでrelease executableを指定します。PATHに追加済みなら `nagic.exe` だけを使えます。

```powershell
cargo build --release --locked -p nagic
./target/release/nagic.exe check examples/sqlite_pool.nagi
./target/release/nagic.exe run examples/sqlite_pool.nagi
```

このsampleは `:memory:` DBを一接続で開き、tableを作り、`7` を保存して読み直し、最後にpoolをcloseします。`query` は0行なら `None`、1行以上なら先頭行を `Some` にします。sampleの `main` が成功したときの出力は `7` と `closed` です。失敗時に `nagic run` がどのerror文面を表示するかはAPI契約ではありません。

sampleの `std.result.map_error` は、異なるerror型をアプリの `AppFailure` に明示変換します。`options` と `bind_f64` は通常の `Error` を返し、DB操作は `sqlite.Failure` を返します。`Failure` は `Error` へ暗黙変換されません。

```nagi
import std.db.sqlite as sqlite
import std.result as result

class Total:
    amount: i64

enum AppFailure:
    Configuration(cause: Error)
    Database(cause: sqlite.Failure)

def configuration_error(cause: Error) -> AppFailure:
    return AppFailure.Configuration(cause)

def database_error(cause: sqlite.Failure) -> AppFailure:
    return AppFailure.Database(cause)

async def write_and_read(pool: view[sqlite.Pool]) -> Result[i64, sqlite.Failure]:
    tx = try await sqlite.begin(pool, sqlite.BeginMode.DEFERRED)
    try await sqlite.exec(tx, sqlite.literal("CREATE TABLE IF NOT EXISTS amounts(amount INTEGER NOT NULL)"), sqlite.parameters())
    values = sqlite.bind_i64(sqlite.parameters(), 7)
    try await sqlite.exec(tx, sqlite.literal("INSERT INTO amounts VALUES (?)"), values)
    try await sqlite.commit(tx)
    tx = try await sqlite.begin(pool, sqlite.BeginMode.DEFERRED)
    row = try await sqlite.query[Total](tx, sqlite.literal("SELECT amount FROM amounts"), sqlite.parameters())
    try await sqlite.rollback(tx)
    match row:
        case Some(total):
            return ok(total.amount)
        case None:
            return ok(0)

async def main() -> Result[unit, AppFailure]:
    config = try result.map_error(sqlite.options(1, 2, 1000, 0), configuration_error)
    pool = try result.map_error(await sqlite.open(":memory:", config), database_error)
    work = await write_and_read(view(pool))
    ending = await sqlite.close(pool, 1000)
    # Observe close even when the transaction failed; preserve the work error first.
    amount = try result.map_error(work, database_error)
    try result.map_error(ending, database_error)
    print(amount)
    return ok(print("closed"))
```

`write_and_read` が失敗しても `main` はcloseをawaitしてからworkのErrを返します。workとcloseが両方失敗した場合、このsampleはwork側を優先しclose側のErrを返しません。業務アプリでは両方をログや独自errorに記録してください。

### 手順が失敗したら

| 症状 | 確認すること |
|---|---|
| `cargo build` がC/native dependencyで失敗 | repository rootか、Rust toolchain・C compiler/linkerが使えるか |
| `nagic: command not found` | `target/release/nagic`（Windowsは `target/release/nagic.exe`）を直接指定するかPATHを確認 |
| `check` が未対応module/APIを示す | 実行中compilerがこのworktreeのソースからbuildされたか。インストール済み0.1.11には本APIがない |
| `check` がrow型を拒否 | `query[T]` / `all[T]` の `T` が標準行readerを生成できるclassか、必要なfield型か |
| `run` がSQLite Failureで終了 | APIが返す `kind` と `message` を確認する。lazy openのため、pathやnative接続の失敗が `begin` で初めて現れることがある |

## 用語と値の所有

| 型 | 役割・主な条件 |
|---|---|
| `Options` | 接続数、各workerのqueue capacity、acquire timeout、SQLite busy timeoutを保持する。constructorで全値を指定する |
| `Query` | 直接literal由来のopaque Copy値。保存・共有・Debug可、Eq/JSON不可 |
| `Pool` | 接続の取得とcloseを管理する。`clone_pool` が同じclose/failure状態を共有するhandleを作る |
| `Tx` | 1接続のtransaction session。非Copyで、同じtaskの中だけで使う。fieldやshared値に格納できない |
| `Parameters` | SQL bind値のowned list。各bind builderは受け取った値をconsumeして新しい `Parameters` を返す |
| `BeginMode` | `DEFERRED`、`IMMEDIATE`、`EXCLUSIVE` |
| `Failure` | DB操作の失敗。`kind`、`outcome`、`retired`、borrowed `message` fieldを持つ |
| `FailureKind` | failureの分類 |
| `Outcome` | failure時に確認できたtransaction結果 |

`Pool` はnon-Copyです。共有handleを増やすときは `sqlite.clone_pool(pool)` を使います。各cloneはclose開始・failureを共有します。`Tx` はnon-Copy/non-Clone/non-sharedで、task境界を越えず、class fieldへ保存できません。同じtask内のlocal、`Option` / `Result`、owned関数委譲では使えます。通常のSQLite operationは `view[Tx]` として暗黙に借ります。自作関数が `view[Tx]` を受け取る場合は、呼出し側で `view(tx)` を明示します。

`commit(tx)` と `rollback(tx)` は引数の `Tx` をconsumeします。Futureを作った時点で所有権が移るため、await後に同じhandleを使えません。読み取りを終えた後でtransactionを続けるなら、古いTxを再利用せず新しく `begin` します。

### 設定とpath

`options(connections, queue_capacity, acquire_ms, busy_ms)` の4引数は省略できません。数値のdefaultはありません。

| 引数 | 条件と意味 |
|---|---|
| `connections` | 正数で、`usize` とTokio Semaphoreのpermit上限に入ること |
| `queue_capacity` | 正数で、同じpermit上限に入ること。**各workerのcommand inbox**の容量で、Pool全体の待機task数やheap使用量の上限ではない |
| `acquire_ms` | 0以上でnative `Duration` / deadlineへ変換できること。Poolの論理slot待ちとnative slot登録を合わせた取得予算 |
| `busy_ms` | 0以上でnative時間範囲内、かつSQLiteの `i32` millisecond範囲内。SQLiteがdatabase lockの解除を待つ時間 |

capacity超過、負数、時間変換範囲外は `options` の `Error` です。0msは「必ずtimeout」ではなく待たない指定です。要求時点で条件が満たされていれば成功します。

`open(path, options)` は空pathと `file:` URIを拒否します。`:memory:` はconnectionごとに別DBになるため、`connections` は1だけです。それ以外は通常のfilesystem pathを使い、相対pathは実行時current working directoryを基準にします。`open` はpathをFuture作成時に所有しますが、native connectionはlazyに `begin` 時に作ります。したがってfilesystem/native openの失敗が `open` ではなく最初の `begin` の `WORKER` failureとして返る場合があります。自動WAL、URI解釈、共有memory URIはありません。

`sqlite.literal("SELECT id FROM items WHERE id = ?")` はcanonical identityにより直接literalだけを受理します。変数・view・連結・format・関数返却strは元位置付きで拒否します。同名のユーザー関数は占有しません。QueryはCopyでlocal/field/返却値に使えますが、SQL文字列を取り出せず、JSON・Eq・直接構築はできません。`query/all/exec`はQuery必須です。通常checkはSQL parserを持たず、SQLのshapeはruntimeまたは明示SQL事前検査が確認します。

### 型付き引数とSQLの形

`parameters()`で空の値列を作り、builderの戻り値を次のbuilderへ渡します。追加した順番が、SQLの匿名placeholder `?` に対応します。

| builder | 追加する値 | 条件 |
|---|---|---|
| `bind_i64(parameters, value)` | `i64` | 整数 |
| `bind_f64(parameters, value)` | `f64` | 有限値のみ。NaN/Infinityは`Error` |
| `bind_text(parameters, value)` | `str` | 所有する文字列を渡す |
| `bind_bytes(parameters, value)` | `bytes` | 所有するbyte列を渡す |
| `bind_null(parameters)` | SQLiteのNULL | 型情報を持つNULLではない |

新APIは一度に一つのSQL文と匿名`?`だけを受け付けます。名前付きの`:name` / `@name`や番号付きの`?1`は使えません。bind数、値型、NULLの読み取りはruntimeで検査します。通常の`check`が実DBの値を使ってSQLを実行することはありません。値はSQL文字列へ連結せず、Parametersで渡します。

### query・all・execの使い分け

| 操作 | 受け付けるSQL | 戻り値 |
|---|---|---|
| `query[T]` | SQLiteがreadonlyと判定し、列を返す一文。`SELECT`という単語だけで判定しない | `Result[Option[T], Failure]`。0行ならNone、それ以外は先頭行のSome |
| `all[T]` | queryと同じreadonlyな行を返すSQL | 全行の`Result[List[T], Failure]` |
| `exec` | 結果の列を返さない一文。通常のtable/index/view/trigger DDLも利用可 | その文の直接変更件数を表す`Result[i64, Failure]` |

`RETURNING`付きの書き込みは、readonlyでもcolumnなしでもないため、上のどの操作も受け付けません。利用者SQLに`BEGIN` / `COMMIT` / `ROLLBACK`とそのvariant、`SAVEPOINT`、`PRAGMA`、`ATTACH`、`DETACH`を書かず、moduleの操作でtransactionを開始・終了します。virtual table、extension、raw connectionの公開はありません。

`T`は標準FromRow生成が対応するfieldを持つclassです。scalarの`i8/i16/i32/i64/u8/u16/u32/f32/f64/bool/str/bytes`、そのOption、対応する`owned[...]` wrapperを使えます。field名を返却列名に合わせ、NULLを受け取るfieldには`T?`を使います。enum、resource、任意generic、未対応fieldは`check`で拒否します。実データの値型・NULL・整数範囲はruntimeのResultで扱います。手書きRustのFromRowを用意しても、新APIのchecker制約は回避できません。

### Public typesと操作一覧

このmoduleは9型と19操作を公開します。`view[T]` は値をborrowする型、`Future[T]` は`await`する非同期処理、`Result[T, E]` は成功値かerrorのどちらかです。`try`は現在のfunctionからErrを返します。より詳しい言語規則は[view](view-and-zero-copy.md)、[async](async.md)、[Result](error-handling.md)を参照してください。

| 型 | Copy / field保存 / shared / debug | 意味 |
|---|---|---|
| `Query` | Copy / storage・shared・Debug可、Eq/JSON不可 | 直接literal由来の固定構造 |
| `Pool` | non-Copy / storage可 / shared可 / Debug可 | `clone_pool`で同じpoolのhandleを増やす |
| `Tx` | non-Copy / storage不可 / shared不可 / Debug不可 | 同じtaskでのみ使うaffine transaction handle |
| `Parameters` | non-Copy / storage可 / shared不可 / Debug不可 | owned bind値をbuilderで積む |
| `Options` | non-Copy / storage可 / shared不可 / Debug可 | `options`で検証して作る設定値 |
| `BeginMode` | Copy・equality可 / storage・shared・Debug可 | `DEFERRED` / `IMMEDIATE` / `EXCLUSIVE` |
| `Failure` | non-Copy / storage・shared・Debug可 | DB error、結果とretirement metadata |
| `FailureKind` | Copy・equality可 / storage・shared・Debug可 | 下表の13分類 |
| `Outcome` | Copy・equality可 / storage・shared・Debug可 | 下表の5状態 |

| 操作 | signature | 使い方と戻り値 |
|---|---|---|
| `literal` | `(sql: str) -> Query` | 直接文字列literalのみをcheckerで受理 |
| `options` | `(connections: i64, queue_capacity: i64, acquire_ms: i64, busy_ms: i64) -> Result[Options, Error]` | 全値を明示し範囲を検証 |
| `open` | `(path: view[str], options: Options) -> Future[Result[Pool, Failure]]` | pathを所有しPoolを作る。native connectionはlazy |
| `clone_pool` | `(pool: view[Pool]) -> Pool` | close/failure状態を共有するhandle |
| `begin` | `(pool: view[Pool], mode: BeginMode) -> Future[Result[Tx, Failure]]` | logical slotを取得してtransactionを始める |
| `parameters` | `() -> Parameters` | 空のbind list |
| `bind_i64` | `(parameters: Parameters, value: i64) -> Parameters` | integerを追加 |
| `bind_f64` | `(parameters: Parameters, value: f64) -> Result[Parameters, Error]` | finite floatだけ追加 |
| `bind_text` | `(parameters: Parameters, value: str) -> Parameters` | owned textを追加 |
| `bind_bytes` | `(parameters: Parameters, value: bytes) -> Parameters` | owned bytesを追加 |
| `bind_null` | `(parameters: Parameters) -> Parameters` | plain SQLite NULLを追加 |
| `query[T]` | `[T](tx: view[Tx], sql: Query, parameters: Parameters) -> Future[Result[Option[T], Failure]]` | row-producing readonly SQLの先頭行 |
| `all[T]` | `[T](tx: view[Tx], sql: Query, parameters: Parameters) -> Future[Result[List[T], Failure]]` | row-producing readonly SQLの全行 |
| `exec` | `(tx: view[Tx], sql: Query, parameters: Parameters) -> Future[Result[i64, Failure]]` | columnを返さない一文 |
| `commit` | `(tx: Tx) -> Future[Result[unit, Failure]]` | transactionを終端してcommit |
| `rollback` | `(tx: Tx) -> Future[Result[unit, Failure]]` | transactionを終端してrollback |
| `close` | `(pool: view[Pool], timeout_ms: i64) -> Future[Result[unit, Failure]]` | closingを開始し、worker joinを待つ |
| `copy_primary_error` | `(problem: view[Failure]) -> Option[Error]` | Failureのprimary causeを明示コピー |
| `copy_cleanup_error` | `(problem: view[Failure]) -> Option[Error]` | cleanup causeを明示コピー |

async operationではawait後に`Result`を`try`または`match`で処理します。`options`と`bind_f64`の失敗は`Error`、残りのDB operationは`Failure`を返します。

#### Parametersの使い方

`bind_i64`などは引数の`Parameters`をmoveして返します。次のbindへ返却値を渡します。`query` / `all` / `exec` もParametersをconsumeするので、同じ値を二回渡せません。

```nagi
def make_values() -> sqlite.Parameters:
    values = sqlite.parameters()
    values = sqlite.bind_i64(values, 4)
    values = sqlite.bind_text(values, "Nagi")
    return values
```

この値はSQLの匿名`?`へ順番にbindされます。queryに渡したあと別文にも値を渡したい場合は、文ごとにParametersを作るか、値を再構築してください。`copy`でParametersを複製することはできません。

#### 複数行を読む

`all[T]`はrow classを指定し、全行を返します。例えば現在の`Total` classでamountを読む処理は次の形です。

```nagi
async def read_all(tx: view[sqlite.Tx]) -> Result[List[Total], sqlite.Failure]:
    return await sqlite.all[Total](tx, sqlite.literal("SELECT amount FROM amounts ORDER BY amount"), sqlite.parameters())
```

`query`は最初の1行だけ、`all`は全行です。readonly SQLの結果を1件だけ扱うなら`query`、複数件を走査するなら`all`を選びます。`exec`はrow結果を返さないDDLやwrite向けです。operation形の誤り、複数文、named/numbered placeholderはruntimeで拒否されます。

## 間違いと直し方

| 間違い | 段階と理由 | 修正 |
|---|---|---|
| `sqlite.query[i64](tx, ...)` | `check`で拒否。新APIは標準FromRow生成を行うclassが必要 | `amount: i64` fieldを持つclassを作って`query[Amount]`を使う |
| `sqlite.exec(tx, sqlite.literal("DELETE FROM items WHERE id = ?1"), params)` | 実行時にSQL failure。新APIは匿名`?`だけ | `?`を使い、Parametersを同じ順にbuilderで作る |
| `sqlite.all[Row](tx, sql, values)`の後も`values`を使う | `all`が`Parameters`をconsumeする | 各SQL用に別のParametersを作る |
| `try await sqlite.commit(tx)`の後で同じ`tx`を再利用 | `commit`はFuture作成時から`Tx`をconsume | 次のSQLは新しい`begin`後のTxで実行 |
| `Tx`をclass fieldへ保存、またはchild taskへmove | `check`で拒否。Txはstorage/shared/task境界を越えない | task内で終端するか、child task内で新しく`begin`する |
| `def read(tx: sqlite.Tx)`へ呼出側の`tx`を渡す | owned parameterへTxをmoveし元から使えなくなる | 引数を`view[sqlite.Tx]`にし、呼出し側で`read(view(tx))`とする |

自作helperは次のようにborrowしたTxを受け取り、同じtask内で使えます。

```nagi
async def read_total(tx: view[sqlite.Tx]) -> Result[Total?, sqlite.Failure]:
    return await sqlite.query[Total](tx, sqlite.literal("SELECT amount FROM amounts"), sqlite.parameters())

async def call_read_total(tx: sqlite.Tx) -> Result[Total?, sqlite.Failure]:
    return await read_total(view(tx))
```

SQLite authorizerはSQL内の `BEGIN` / `COMMIT` / `ROLLBACK`、`SAVEPOINT`、`PRAGMA`、`ATTACH` / `DETACH` も拒否します。transaction境界にはmodule operationを使います。

## transactionと並行処理の境界

Poolのlogical slot待ちはTokioのFIFO semaphoreへ委譲されます。FIFOの説明はこの**logical permit wait**に限ります。native workerの起動、native slot登録後のadmission、SQLite `BEGIN` 完了までの順序は保証しません。

`acquire_ms` はlogical permit待ちからnative record登録まで同じ取得予算を使います。connection startupと登録後のready/`BEGIN` SQL busy waitはこの予算に含みません。`busy_ms` がSQLite busy waitを制限します。SQL実行・commitに別のinterrupt deadlineはありません。

一つの `Tx` が一接続を専有します。Pool cloneは共有しても、Txをchild taskへmove/captureしたり、別taskから同じTxを使ったりできません。必要ならchild task内でPoolから独自に `begin` します。外側taskの `TaskFailure`（子自体の失敗）と、そのtaskの戻り値に入る `Result[..., sqlite.Failure]`（DB操作の失敗）は別の層です。

普通のSQL、bind、decode errorでもnative transactionがactiveだと確認できればTxは継続できます。ただし失敗したstatementが変更を一切していないとは限らず、同じtransaction内の先行効果が残る場合があります。SQLiteが自動rollbackした場合は `ABORTED` / `ROLLED_BACK` として後続操作を拒否します。cancelは受理済みSQLや外部副作用をundoしません。retryやrollbackが安全だと推測せず、業務のidempotencyと明示rollbackを設計してください。

## Failureを読む

Failureの分類とtransaction結果を分けて読みます。`outcome` は観測できた状態であり、再実行の安全性を表しません。

| `FailureKind` | 意味 |
|---|---|
| `INVALID` | path/optionsやtimeout設定が不正 |
| `CLOSED` | Poolがclosingまたはclosed |
| `ACQUIRE_TIMEOUT` | 取得予算内にlogical/native slotを確保できない |
| `BUSY` | SQLite database lockがbusy/locked |
| `SQL` | SQLのprepare/実行に失敗 |
| `BIND` | placeholder/value bindingの失敗 |
| `DECODE` | SQLite rowからclass fieldへの変換失敗 |
| `ABORTED` | SQLite native transactionが既に終了した |
| `CLEANUP` | transaction/connection cleanupの失敗 |
| `WORKER` | native connection起動またはworker処理の失敗 |
| `REPLY_LOST` | workerから結果を受け取れず、結果が不明 |
| `CLOSE_TIMEOUT` | close期限までに全worker joinを確認できない |
| `ALLOCATION` | fallible ledger/idle reservationに失敗 |

`ALLOCATION` は明示的なfallible reservation失敗を分類します。すべてのOOMやallocator abortから回復する保証ではありません。

| `Outcome` | 意味 |
|---|---|
| `NOT_APPLICABLE` | このfailureに関連するactive Txがない、またはまだ存在しない |
| `ACTIVE` | failure時点でnative Txがactiveだと確認できた |
| `COMMITTED` | commit完了を確認した |
| `ROLLED_BACK` | rollbackまたはSQLiteのautomatic rollbackを確認した |
| `UNKNOWN` | 完了状態を確定できない |

`retired` はそのFailureで接続のretirementが確認されたかを表します。`false` は「このfailureからretirementを確認できていない」だけで、workerやPoolが健全だと証明しません。特に `REPLY_LOST` + `UNKNOWN` から、rollback済み・connection再利用可能・retry安全を推論しないでください。Debug表示は `kind` / `outcome` / `retired` に限定されますが、公開 `message` の文字列は安定したprotocolではありません。

`Failure.message` は `Failure` ownerを借りる `view[str]` です。Failureより長く保持するなら `copy(problem.message)` で所有文字列を作ります。native causeをerrorとして保持したいときは `copy_primary_error(problem)` と `copy_cleanup_error(problem)` を明示的に使います。どちらも `Option[Error]` で、causeがなければ `None`、ある場合はkind/messageをコピーします。commitが完了してcleanupが失敗するなど、primaryとcleanupの情報が別々に存在するfailureがあります。

## Poolを閉じる

`close(pool, timeout_ms)` はcloneすべてで共有するclosingを開始し、新しい `begin` を止め、active Txの終了とworkerのnative close/actual joinを待ちます。timeoutが0でも既に終了を確認済みなら成功できます。timeoutやclose Futureのcancel後もclosingは戻りません。active Txを強制終了せず、後の `close` でも完了を待てます。

`Pool` の最後のhandleをDropしても、非同期cleanupやworker joinの完了確認にはなりません。Dropは閉鎖を要求するだけです。終了を確認する必要がある処理では、所有者が `close` をawaitしてください。close成功はnative closeとworker threadのactual joinを確認した結果です。無期限にblockするSQL、process kill、allocator abortからcloseが必ず戻る保証はありません。

## SQLの事前検査

通常checkはQueryのliteral境界、型・所有権・行fieldを検査し、SQLiteエンジンを起動しません。明示schemaによるopt-in検査は直接Query constructorのSQL形・schema・必要列をprepare-onlyで確認します。直接Parameters builder列のbind個数は照合し、変数/関数経由のQuery構造やParameters個数はruntime検査として表示します。実値型・NULL・数値範囲・DB内データはruntimeで確認します。[SQL事前検査](sql-check.md)を参照してください。

## 使い分けと関連ページ

- 汎用typed parameters、複数文を跨ぐ明示Tx、複数接続とclose制御が必要なら、この新APIを選びます（使えるのはこのAPIを含む開発sourceからbuildしたcompiler）。
- API shapeをschemaと照合するoffline checkは[SQL事前検査](sql-check.md)。Nagi `Result`、`try`、nullable、`view`、ownershipの規則は[エラー処理](error-handling.md)、[Optionとnullable](types.md)、[所有権](ownership.md)、[view](view-and-zero-copy.md)、[async](async.md)を参照してください。
