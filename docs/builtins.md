# 組み込み関数

[目次](README.md) · [入門ガイド](language-guide.md) · [文法の早見表](syntax.md)

ここにある関数はimportなしで使えます。表の`T`は、数値などの対応する型に読み替えてください。利用者が型引数を持つ関数を定義する機能は未対応です。

## 出力、入力、環境

| 呼び出し | 引数の型（順番どおり） | 戻り値 | 使い方・注意 |
|---|---|---|---|
| `print(value)` | 数値・bool・str・view[str]・UUID | `unit` | 1つの数値・bool・文字列などを改行付きで表示。classや配列をそのまま表示するAPIはない |
| `write(value)` | printと同じ | `unit` | 改行なしで表示。引数はprintと同じ |
| `read_line()` | なし | `Result[str, Error]` | `text = try read_line()`。コンソールの同期入力 |
| `env("NAME", "default")` | str、str | `str` | 環境変数が取得できなければ既定値 |
| `assert_true(condition)` | bool | `unit` | 条件はbool。Falseならpanic |

`read_line`は出力を画面へ反映してから1行読みます。末尾の改行を除き、ほかの空白は残します。入力が終わると空文字列を返します。入力中は処理が止まるため、コンソールアプリで使います。

## 文字列、配列、借用

| 呼び出し | 引数の型（順番どおり） | 戻り値 | 使い方・注意 |
|---|---|---|---|
| `len(value)` | str・bytes・List[T]とそのview | `i64` | str / bytesはbyte数、Listは要素数。viewも対応 |
| `range(end)` | i64 | for用の範囲 | `for i in range(3):`。i64の終端を1つ指定、0から終端未満 |
| `append(list, value)` | List[T]の変数、T | `unit` | List変数の末尾に同じ型の値を追加 |
| `view(text)` | str | `view[str]` | 所有文字列を読み取り用に借りる |
| `view(bytes)` | bytes | `view[bytes]` | 所有byte列を借りる |
| `view(list)` | List[T] | `view[T]` | `List[T]`を借りる。`view[List[T]]`とは書かない |
| `copy(borrowed)` | view[str]・view[bytes]・view[T] | 所有str / bytes / List | viewから独立した所有コピーを作る |
| `slice(borrowed, start, end)` | view、i64、i64 | `Result[view[...], Error]` | `try slice(view(text), 0, 3)`。終端は含まない |

sliceの位置はi64です。str / bytesの位置はbyte単位、Listの位置は要素単位。範囲外や文字列のUTF-8境界の誤りはResultの失敗になります。viewが生きている元データはmove・再代入・appendできません。

## 変換、成功、失敗

| 呼び出し | 引数の型（順番どおり） | 戻り値 | 使い方・注意 |
|---|---|---|---|
| `parse_i64(text)` | str・view[str] | `Result[i64, Error]` | str / view[str]を整数へ。`try parse_i64("42")` |
| `parse_f64(text)` | str・view[str] | `Result[f64, Error]` | str / view[str]を小数へ |
| `i64(value)` | i8・i16・i32・u8・u16・u32 | `i64` | i8 / i16 / i32 / u8 / u16 / u32の損失のない拡張 |
| `i32(value)` | i64 | `Result[i32, Error]` | i64を範囲検査して縮小。`try i32(value)` |
| `ok(value)` | T | `Result[T, Error]` | 成功を返す。`return ok(value)` |
| `error("理由")` | str | 文脈に合う`Result[T, Error]` | 入力の失敗。HTTPでは400 |
| `not_found("理由")` | str | 文脈に合う`Result[T, Error]` | 対象なし。HTTPでは404 |
| `internal_error("理由")` | str | 文脈に合う`Result[T, Error]` | 内部の失敗。HTTPでは詳細を伏せた500 |
| `fail(problem)` | Error | 文脈に合う`Result[T, Error]` | Errorをmoveして、元のkindとmessageを保って返す |
| `error_kind(problem)` | Error | `str` | Errorを借りて種類を取得。例：`invalid`、`database` |
| `error_message(problem)` | Error | `str` | Errorを借りてmessageをコピー |
| `some(value)` | T | `T?` | nullableの値ありを作る。値なしは`None` |
| `uuid_parse(text)` | str・view[str] | `Result[UUID, Error]` | 文字列からUUIDへ |
| `uuid_format(value)` | UUID | `str` | UUIDから文字列へ |

`try`は関数ではなく構文です。Resultを返す関数の中で成功値を取り出し、失敗なら呼び出し元へ返します。その場で処理する場合は`match`と`case Ok(value)` / `case Err(problem)`を使います。

Errorを作る関数はメッセージの所有文字列を受け取り、`fail`はErrorを消費します。`error_kind`と`error_message`はErrorを消費しません。成功型は戻り先などのResult型から決まり、型の文脈がなければ`Result[unit, Error]`です。詳しくは[エラー処理](error-handling.md)を参照してください。

## JSON、HTML

| 呼び出し | 引数の型（順番どおり） | 戻り値 | 使い方・注意 |
|---|---|---|---|
| `json_decode[User](input)` | str・bytes・view[str]・view[bytes] | `Result[User, Error]` | 型付きclassへ読む。入力はstr / bytesとそのview |
| `json_encode(value)` | JSONに変換できる型（[詳細](json.md)） | `Result[str, Error]` | JSON文字列へ変換。型引数は付けない |
| `html(text)` | str | `Html` | strをHTML応答用にする。入力文字列の所有権を受け取る |
| `include_text("index.html")` | 文字列リテラル | `str` | ソースに隣接するUTF-8ファイルをコンパイル時に埋め込む |

`json_decode`の`[User]`は型引数です。`json_decode(User, input)`ではありません。include_textのパスは文字列リテラルで指定します。詳細は[JSON](json.md)と[HTTP](http.md)にあります。

## 非同期、HTTP、SQLite

次の関数は非同期です。表には**awaitした後の型**を記載しています。Resultから値を取り出すときは`try await ...`、そのままResultを返すなら`return await ...`です。

| 呼び出し | 引数の型（順番どおり） | await後の型 | 用途 |
|---|---|---|---|
| `sleep(milliseconds)` | i64 | `unit` | i64のミリ秒数だけ待つ |
| `db_open(path)` | str・view[str] | `Result[Db, Error]` | SQLiteを開く。`:memory:`ならメモリ内 |
| `serve(db, port)` | Db、i64 | `Result[unit, Error]` | loopbackのHTTPサーバーを起動。Dbの所有権を受け取る |
| `db_exec(db, sql)` | Db、str・view[str] | `Result[i64, Error]` | SQLを実行、影響した行数 |
| `db_all[User](db, sql)` | Db、str・view[str] | `Result[List[User], Error]` | 複数行を読む。現在はbind引数なし |
| `db_query[User](db, sql, id)` | Db、str・view[str]、i64 | `Result[User?, Error]` | 1行を読む。i64のbind引数を1つ |
| `db_write(db, sql, id)` | Db、str・view[str]、i64 | `Result[i64, Error]` | i64のbind引数を1つ、影響した行数 |
| `db_insert[User](db, sql, text, number)` | Db、str・view[str]、str、i32 | `Result[User, Error]` | bind引数はstr、i32の2つ |
| `db_update[User](db, sql, id, text, number)` | Db、str・view[str]、i64、str、i32 | `Result[User, Error]` | bind引数はi64、str、i32の3つ |

SQLとpathはstr / view[str]です。portはi64。insert / updateのtextは所有strで、workerへmoveします。insert / updateのSQLには`RETURNING`を付け、指定classに合う列を返してください。SQLのAPIは現時点では固定の引数形です。詳細は[SQLite](database.md)を参照してください。

## 共有と型のサイズ

| 呼び出し | 引数の型 | 戻り値 | 動作・制約 |
|---|---|---|---|
| `share(value)` | T | `shared[T]` | 所有権を受け取り、共有する値を作る |
| `clone_shared(value)` | shared[T] | `shared[T]` | 同じ値への共有参照を増やす。データ全体はコピーしない |
| `size_of[Point]()` | なし。型引数を1つ指定 | `i64` | 型のサイズをバイト単位で返す。文字列・配列が別に確保する領域は含まない |

値の扱いは[メモリの扱い](memory-model.md)、classのサイズの例は[class](classes.md)を参照してください。

## 検証・計測用の関数

次の関数はランタイムの試験と性能測定に使います。`actor_demo`などは決まった処理を実行するサンプルで、利用者のactorやworkerを登録するAPIではありません。

非同期関数の表にはawait後の型を記載しています。

| 呼び出し | 引数の型 | await後の型 | 試験の内容 |
|---|---|---|---|
| `actor_demo(count)` | i64 | `Result[i64, Error]` | カウンターへメッセージを送る |
| `actor_pair_demo(count)` | i64 | `Result[i64, Error]` | 中継役とカウンター役が通信する |
| `supervisor_demo()` | なし | `Result[i64, Error]` | panicしたworkerを再起動する |
| `queue_demo(count)` | i64 | `Result[i64, Error]` | キューへ仕事を渡し、失敗時に再試行する |
| `task_demo(count)` | i64 | `Result[i64, Error]` | 複数の子taskを起動して終了を待つ |
| `cpu_sum(count)` | i64 | `Result[i64, Error]` | CPU処理を別の処理枠で実行する |

詳細は[actor](actor.md)、[workerの再起動](supervisor.md)、[キュー](queue.md)、[並行処理](concurrency.md)と、対応する`examples/`を参照してください。

次の関数は同期関数です。

| 呼び出し | 引数の型（順番どおり） | 戻り値 | 動作 |
|---|---|---|---|
| `clock_ns()` | なし | `i64` | 計測用の時刻をナノ秒単位で返す |
| `make_ints(count)` | i64 | `List[i64]` | CPU試験用の整数配列を作る |
| `bench_i64(name, count, kernel)` | str、i64、view[i64]を受けてi64を返す関数 | `unit` | 整数配列の処理を測定する |
| `bench_f64(name, count, kernel)` | str、i64、view[f64]を受けてf64を返す関数 | `unit` | 小数配列の処理を測定する |
| `bench_scalar(name, count, kernel)` | str、i64、i64を受けてi64を返す関数 | `unit` | 配列を使わない整数処理を測定する |

`kernel`には同期関数を渡します。測定方法と数値の単位は[性能の読み方](performance.md)、実行例は[CPU試験](../examples/cpu.nagi)にあります。
