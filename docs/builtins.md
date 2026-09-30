# よく使う関数

[目次](README.md) · [入門ガイド](language-guide.md) · [文法の早見表](syntax.md)

これらはimportなしで使える組み込み関数です。表の`T`は対応する要素型を表す説明用の記号です。自作関数にgenericを定義できるという意味ではありません。

## 出力、入力、環境

| 呼び出し | 戻り値 | 使い方・注意 |
|---|---|---|
| `print(value)` | `unit` | 1つの数値・bool・文字列などを改行付きで表示。classや配列をそのまま表示するAPIはない |
| `write(value)` | `unit` | 改行なしで表示。引数はprintと同じ |
| `read_line()` | `Result[str, Error]` | `text = try read_line()`。コンソールの同期入力 |
| `env("NAME", "default")` | `str` | 環境変数が取得できなければ既定値 |
| `assert_true(condition)` | `unit` | 条件はbool。Falseならpanic |

`read_line`は標準出力をflushしてから1行読み、末尾のCR/LFを除きます。空白は保持し、EOFでは空文字列を返します。threadをブロックするので、HTTP handlerではなくコンソールアプリ向けです。

## 文字列、配列、借用

| 呼び出し | 戻り値 | 使い方・注意 |
|---|---|---|
| `len(value)` | `i64` | str / bytesはbyte数、Listは要素数。viewも対応 |
| `range(end)` | for用の範囲 | `for i in range(3):`。i64の終端を1つ指定、0から終端未満 |
| `append(list, value)` | `unit` | List変数の末尾に同じ型の値を追加 |
| `view(text)` | `view[str]` | 所有文字列を読み取り用に借りる |
| `view(bytes)` | `view[bytes]` | 所有byte列を借りる |
| `view(list)` | `view[T]` | `List[T]`を借りる。`view[List[T]]`とは書かない |
| `copy(borrowed)` | 所有str / bytes / List | viewから独立した所有コピーを作る |
| `slice(borrowed, start, end)` | `Result[view[...], Error]` | `try slice(view(text), 0, 3)`。終端は含まない |

sliceの位置はi64です。str / bytesの位置はbyte単位、Listの位置は要素単位。範囲外や文字列のUTF-8境界の誤りはResultの失敗になります。viewが生きている元データはmove・再代入・appendできません。

## 変換、成功、失敗

| 呼び出し | 戻り値 | 使い方・注意 |
|---|---|---|
| `parse_i64(text)` | `Result[i64, Error]` | str / view[str]を整数へ。`try parse_i64("42")` |
| `parse_f64(text)` | `Result[f64, Error]` | str / view[str]を小数へ |
| `i64(value)` | `i64` | i8 / i16 / i32 / u8 / u16 / u32の損失のない拡張 |
| `i32(value)` | `Result[i32, Error]` | i64を範囲検査して縮小。`try i32(value)` |
| `ok(value)` | `Result[T, Error]` | 成功を返す。`return ok(value)` |
| `error("理由")` | 文脈に合うResult | 失敗を返す。`return error("invalid name")` |
| `some(value)` | `T?` | nullableの値ありを作る。値なしは`None` |
| `uuid_parse(text)` | `Result[UUID, Error]` | 文字列からUUIDへ |
| `uuid_format(value)` | `str` | UUIDから文字列へ |

`try`は関数ではなく構文です。Resultを返す関数の中で成功値を取り出し、失敗なら呼び出し元へ返します。詳しくは[エラー処理](error-handling.md)を参照してください。

## JSON、HTML

| 呼び出し | 戻り値 | 使い方・注意 |
|---|---|---|
| `json_decode[User](input)` | `Result[User, Error]` | 型付きclassへ読む。入力はstr / bytesとそのview |
| `json_encode(value)` | `Result[str, Error]` | JSON文字列へ変換。型引数は付けない |
| `html(text)` | `Html` | strをHTML応答用にする。入力文字列の所有権を受け取る |
| `include_text("index.html")` | `str` | ソースに隣接するUTF-8ファイルをコンパイル時に埋め込む |

`json_decode`の`[User]`は型引数です。`json_decode(User, input)`ではありません。include_textのパスは文字列リテラルで指定します。詳細は[JSON](json.md)と[HTTP](http.md)にあります。

## 非同期、HTTP、SQLite

次の関数は非同期です。表には**awaitした後の型**を記載しています。Resultから値を取り出すときは`try await ...`、そのままResultを返すなら`return await ...`です。

| 呼び出し | await後の型 | 用途 |
|---|---|---|
| `sleep(milliseconds)` | `unit` | i64のミリ秒数だけ待つ |
| `db_open(path)` | `Result[Db, Error]` | SQLiteを開く。`:memory:`ならメモリ内 |
| `serve(db, port)` | `Result[unit, Error]` | loopbackのHTTPサーバーを起動。Dbの所有権を受け取る |
| `db_exec(db, sql)` | `Result[i64, Error]` | SQLを実行、影響した行数 |
| `db_all[User](db, sql)` | `Result[List[User], Error]` | 複数行を読む。現在はbind引数なし |
| `db_query[User](db, sql, id)` | `Result[User?, Error]` | 1行を読む。i64のbind引数を1つ |
| `db_write(db, sql, id)` | `Result[i64, Error]` | i64のbind引数を1つ、影響した行数 |
| `db_insert[User](db, sql, text, number)` | `Result[User, Error]` | bind引数はstr、i32の2つ |
| `db_update[User](db, sql, id, text, number)` | `Result[User, Error]` | bind引数はi64、str、i32の3つ |

SQLとpathはstr / view[str]です。portはi64。insert / updateのtextは所有strで、workerへmoveします。insert / updateのSQLには`RETURNING`を付け、指定classに合う列を返してください。SQLのAPIは現時点では固定の引数形です。詳細は[SQLite](database.md)を参照してください。

## その他

`share(value)`で明示的な共有所有値`shared[T]`を作り、`clone_shared(value)`で共有参照を複製できます。`size_of[Point]()`は型のサイズをi64で返します。

actor / Supervisor / queueや計測用の組み込み関数は、[actor](actor.md)、[Supervisor](supervisor.md)、[queue](queue.md)、[性能](performance.md)と対応する`examples/`を参照してください。これらには試験専用のAPIもあり、汎用のアプリ向け標準ライブラリとしては未完成です。
