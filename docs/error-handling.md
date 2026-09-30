# Resultで失敗を扱う

[目次](README.md) · [文法](syntax.md) · [関数一覧](builtins.md)

`Result[T, Error]`は成功値`T`か失敗`Error`を返します。失敗を呼び出し元へ伝えるときは`try`、その場で回復したり別の応答にしたりするときは`match`を使います。

## 失敗を呼び出し元へ返す

次は関数の例です。`try`で成功値を取り出し、失敗ならそのErrorをそのまま返します。`try`を書く関数自身もResultを返す必要があります。

```nagi
def read_id(text: view[str]) -> Result[i64, Error]:
    id = try parse_i64(text)
    if id < 1:
        return error("id must be positive")
    return ok(id)
```

非同期の処理では`value = try await operation(...)`と書きます。

## 成功と失敗を分ける

次は完全なコードです。[result.nagi](../examples/tutorial/result.nagi)にもあります。失敗したら既定値に回復するため、この関数の戻り値は通常の`i64`にできます。

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

リポジトリのルートで実行します。

```powershell
.\target\release\nagic.exe run examples/tutorial/result.nagi
```

出力は順に`21`、`invalid`、`-1`です。`Ok`と`Err`は先頭が大文字のパターンです。値を作る関数は小文字の`ok(...)`や`error(...)`を使います。

- `case Ok(...)`と`case Err(...)`を、それぞれ1回ずつ書く。順番はどちらでもよい。
- 括弧内の名前には成功値・失敗値の型が付く。使わない値は`case Err(_):`などと書く。
- 名前はそのcase内だけで使える。外側で使っている変数名との重複は拒否する。
- matchは対象のResultを消費する。所有文字列などのpayloadもmoveされる。借用payloadの元データはcase内でも借用中として検査する。
- 両方のcaseがreturnすれば、関数の全経路で値を返すものとして検査する。

現在のmatchはResultを対象とする文です。値を返すmatch式、nullableの`Some` / `None`、ガード、入れ子のパターンは未対応です。[Low](low-language.md)でも同じ分岐を使えます。

## Errorを調べる・作る・返し直す

| 書き方 | 意味 |
|---|---|
| `error("理由")` | 入力の失敗を作る。kindは`invalid` |
| `not_found("理由")` | 対象なしの失敗を作る。kindは`not_found` |
| `internal_error("理由")` | 内部の失敗を作る。kindは`internal` |
| `error_kind(problem)` | kindの名前を所有文字列で取得する |
| `error_message(problem)` | messageのコピーを所有文字列で取得する |
| `return fail(problem)` | 元のkindとmessageを保ち、Errorをmoveして返す |

Errorを作る関数と`fail`の成功型は、戻り先や変数の型から決まります。型の文脈がなければ`Result[unit, Error]`です。`error_kind`と`error_message`はErrorを消費しないため、そのあとで`fail(problem)`を使えます。messageにはDBなどの内部情報が含まれる場合があります。

HTTPではErrorの種類を次のように変換します。

| kind | HTTPステータス |
|---|---|
| `invalid` | 400 |
| `not_found` | 404 |
| `busy` | 503 |
| `database` / `internal` | 500 |

DB・内部エラーの500応答は`{"error":"internal error"}`で、詳細はサーバーのログに出します。`Result[T?, Error]`の成功値が`None`でも404になります。

入力不正・対象なし・DB失敗と、失敗からの回復を試すには[Result APIサンプル](../test-nagi-code/result-api/README.md)を使ってください。Pythonのsmokeは起動済みサーバーにHTTPリクエストを送り、応答を照合します。

## 検査とpanicの範囲

直接捨てたResult、awaitしていないFutureはcheckerが拒否します。代入したResultを全経路で必ず処理する検査は未完成です。

JSON・DB・入力検査の失敗とpanicは別です。scopeでは子taskのpanicを検出し、Supervisorではworkerのpanicを再起動対象にします。メモリ破壊・process abortの回復機構ではありません。

診断はsource path、行、該当ソース、理由とhelpを表示します。診断のsource spanは行中心です。VS Codeの定義ジャンプは元ソースの列位置も扱いますが、Rust backendの全診断をHighの厳密な列位置へ戻すsource mapは未実装です。
