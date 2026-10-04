# HTTP

`std.http.server`でHTTPサーバーを作れます。DBは不要です。asyncのhandler、共有する状態、失敗時の応答をNagiで定義します。

## 最小のサーバー

次を`server.nagi`に保存します。

```nagi
import std.http.server as http

class State:
    greeting: str

async def hello(request: http.Request, state: shared[State]) -> Result[http.Response, Error]:
    return ok(http.text(http.Status.OK, view(state.greeting)))

async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(greeting="Hello, Nagi!"))
    app = try http.route(app, http.Method.GET, "/", hello)
    return await http.serve(app, 8080, http.default_options())
```

```sh
nagic run server.nagi
```

[http://127.0.0.1:8080/](http://127.0.0.1:8080/)を開くと`Hello, Nagi!`が返ります。Ctrl+Cで停止します。

## リクエストと応答

| 操作 | 書き方 |
|---|---|
| GETか調べる | `request.is_get` または `request.method == http.Method.GET` |
| パスを読む | `request.path` |
| bodyを借りる | `request.body` |
| ヘッダーを読む | `http.header_text(view(request), "Authorization")` |
| 文字列を返す | `http.text(http.Status.OK, "hello")` |
| HTMLを返す | `http.html(http.Status.OK, "<h1>Hello</h1>")` |
| JSONを返す | `http.json[User](http.Status.CREATED, user)` |
| bodyのない応答 | `http.empty(http.Status.NO_CONTENT)` |

ヘッダーは存在しないこともあるため、`Result[Option[view[str]], Error]`を返します。`match`の`Ok`／`Err`と`Some`／`None`で分岐します。POSTも`http.Method.POST`で登録できます。

## エラー処理を変える

`http.app[State, AuthError](state, auth_error)`で、独自のエラー型から応答へ変換する関数を指定できます。`http.route_mapped`を使うと、そのrouteだけ別の変換関数を使います。正常な応答は変換しません。

[認証サンプル](../test-nagi-code/library-examples/http-auth/README.md)には、Authorizationの読み取り、401とWWW-Authenticate、routeごとの403、型付き状態を含む実行例があります。

## 次に読む

- [HTTP APIリファレンス](http-server.md)：Status、ヘッダー、route、制限の設定
- [標準HTTPの測定結果](http-stdlib-performance.md)：応答速度、メモリ、連続負荷
- [JSON](json.md)：bodyをclassへ変換する
- [既存のHTTP属性](http-legacy.md)：`@get`／`@post`と`serve(Db, port)`を使うコード

現在の標準サーバーはloopbackのHTTP/1.1に対応します。TLSや外部公開にはリバースプロキシを使います。接続元IPの取得、ストリーミング、WebSocket、HTTP/2の公開APIはありません。外部APIへリクエストを送る標準HTTP clientも未実装です。

内部ではHyperがHTTP通信、Tokioが非同期実行を担います。Nagiはroute登録、型付き状態、Response、制限とエラー処理のAPIを提供します。Axum／Towerへの変更は比較検討中で、現行サーバーの置き換えや性能改善が決まったわけではありません。
