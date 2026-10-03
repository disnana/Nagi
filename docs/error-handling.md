# Resultで失敗を扱う

[目次](README.md) · [文法](syntax.md) · [関数一覧](builtins.md)

`Result[T, E]`は成功値`T`か失敗値`E`を返します。`E`には組み込みの`Error`や、自分で定義したclass・enumを使えます。失敗を呼び出し元へ返すには`try`、その場で処理するには`match`を使います。

## 失敗を呼び出し元へ返す

`try`は成功値を取り出し、失敗ならその値を返します。呼び出し先と自分の戻り値は、同じエラー型`E`である必要があります。

```nagi
def read_id(text: view[str]) -> Result[i64, Error]:
    id = try parse_i64(text)
    if id < 1:
        return error("id must be positive")
    return ok(id)
```

非同期では`value = try await operation(...)`です。`try`の結果は`T`なので、Resultとして返すなら`return ok(try operation(...))`と書きます。異なる`E`への変換は`match`の`Err`で明示します。

## 成功と失敗を分ける

失敗から既定値へ回復すれば、通常の`i64`を返せます。[result.nagi](../examples/tutorial/result.nagi)は次の完全な例です。

```nagi
def number_or(text: str, fallback: i64) -> i64:
    match parse_i64(view(text)):
        case Ok(number):
            return number
        case Err(problem):
            print(error_kind(problem))
            message = error_message(problem)
            assert_true(len(view(message)) > 0)
            return fallback

def main():
    print(number_or("21", 0))
    print(number_or("oops", -1))
```

`result.nagi`として保存したフォルダーで実行します。

```powershell
nagic run result.nagi
```

出力は順に`21`、`invalid`、`-1`です。`Ok`と`Err`は先頭が大文字のパターンです。値を作る関数は小文字の`ok(...)`や`error(...)`を使います。

- `case Ok(...)`と`case Err(...)`を、それぞれ1回ずつ書く。順番はどちらでもよい。
- 括弧内の名前には成功値・失敗値の型が付く。使わない値は`case Err(_):`などと書く。
- 名前はそのcase内だけで使える。外側で使っている変数名との重複は拒否する。
- matchは対象のResultを消費する。所有文字列などのpayloadもmoveされる。借用payloadの元データはcase内でも借用中として検査する。
- 両方のcaseがreturnすれば、関数の全経路で値を返すものとして検査する。

matchはResult・Option・enumを対象とする文です。Optionは`case Some(value):`と`case None:`の両方を書きます。match式、ガード、入れ子のパターン、`case _`は未対応です。[Low](low-language.md)でも同じ分岐を使えます。

## 独自のエラー型

enumは失敗の種類と、それぞれに必要な情報をまとめます。

```nagi
enum QuantityError:
    InvalidNumber
    OutOfRange(minimum: i64)

def positive(value: i64) -> Result[i64, QuantityError]:
    if value < 1:
        return fail(QuantityError.OutOfRange(minimum=1))
    return ok(value)

def fallback(problem: QuantityError) -> i64:
    match problem:
        case QuantityError.InvalidNumber:
            return 0
        case QuantityError.OutOfRange(minimum):
            return minimum
```

値だけの種類は`QuantityError.InvalidNumber`、情報を持つ種類は`QuantityError.OutOfRange(1)`または名前付き引数で作ります。caseには全種類を1回ずつ書きます。payloadは位置順に受け取り、不要な値は`_`にします。未知の種類、不正な引数、caseの不足・重複は`check`で拒否します。

classも`Result[T, MyError]`の失敗値に使えます。たとえば`class StorageError:`のフィールドに`cause: Error`を持たせ、`fail(StorageError(cause=problem))`で元の原因を保存できます。class・enumのエラー型にJSONやDBの変換は要求しません。組み込みErrorを含むclassやenumのJSON変換は未対応です。

[独自エラーのCLIサンプル](../test-nagi-code/library-examples/typed-errors/README.md)は、組み込みErrorからの変換、同じエラー型の`try`、enumの分岐を試せます。

## Errorを調べる・作る・返し直す

| 書き方 | 意味 |
|---|---|
| `error("理由")` | 入力の失敗を作る。kindは`invalid` |
| `not_found("理由")` | 対象なしの失敗を作る。kindは`not_found` |
| `internal_error("理由")` | 内部の失敗を作る。kindは`internal` |
| `error_kind(problem)` | kindの名前を所有文字列で取得する |
| `error_message(problem)` | messageのコピーを所有文字列で取得する |
| `return fail(problem)` | 元のkindとmessageを保ち、Errorをmoveして返す |

成功型は戻り値や変数の型から決まります。`fail(problem)`は組み込みErrorだけでなく、class・enumの失敗値もmoveします。型の文脈がなければ`Result[unit, E]`です。`error_kind`と`error_message`は組み込みError専用で、元の値を消費しません。messageにはDBなどの内部情報が含まれる場合があります。

従来の`@get`などのHTTPハンドラーは`Result[..., Error]`を返します。Nagi 0.1.8から使える[`std.http.server`](http.md)では独自のエラー型`E`を使い、AppまたはrouteのmapperでResponseへ変換します。従来のハンドラーでの組み込みErrorの変換は次のとおりです。

| kind | HTTPステータス |
|---|---|
| `invalid` | 400 |
| `not_found` | 404 |
| `busy` | 503 |
| `database` / `internal` | 500 |

DB・内部エラーの500応答は`{"error":"internal error"}`で、詳細はサーバーのログに出します。`Result[T?, Error]`の成功値が`None`でも404になります。

入力不正・対象なし・DB失敗と、失敗からの回復を試すには[Result APIサンプル](../test-nagi-code/result-api/README.md)を使ってください。サンプルのテストは、起動したサーバーにリクエストを送り、応答を確認します。

## 検査とpanicの範囲

直接捨てたResult、awaitしていないFutureは型検査で拒否します。代入したResultを全経路で必ず処理する検査は未完成です。

Resultの失敗とpanicは別です。scopeは子taskのpanicを検出します。Nagi 0.1.8から使える[`std.actor`](actor.md)では、`Turn`内の業務エラー`E`は次の状態を保存して返信し、handler自身のErrorやpanicにはSupervisorの再起動方針を適用します。`call`は`Result[Result[R, E], CallError]`を返すため、業務エラーと未起動・停止・タイムアウトなどを分けて扱います。[サンプル](../test-nagi-code/library-examples/supervised-service/README.md)でHTTP応答への変換も試せます。

旧`supervisor_demo`は固定workerの再起動を試す検証用APIです。どちらもメモリ破壊やprocess abortを回復する機構ではありません。

診断はファイル名、行、該当ソース、理由を表示します。ビルド時も、元の位置を特定できるエラーはNagi・Lowの文や定義の行を先に表示し、生成Rustの詳しい診断を続けます。Rustの修正候補はRust向けなので、そのままNagiへ適用しないでください。手書きRustや位置を特定できない診断はRust側の表示を使います。厳密な列位置や全Rust診断の対応は未実装です。VS Codeの定義ジャンプは元ソースの列位置も扱います。
