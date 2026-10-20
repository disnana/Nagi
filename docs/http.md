# HTTP

このページは0.2.0向けです。公開済み0.1.xのコードは[移行ガイド](migration-0.2.0.md)を確認してください。routeはpublicを含めて明示Policyが必要です。

`std.http.server`でHTTPサーバーを作れます。DBは不要です。asyncのhandler、共有する状態、失敗時の応答をNagiで定義します。

## 最小のhandler

次はhandlerの断片です。SF07の完全な起動例ではありません。

```nagi
import std.http.server as http

class State:
    greeting: str

async def hello(request: http.Request, state: shared[State], access: unit) -> Result[http.Response, Error]:
    context = http.response_context(view(request))
    return http.text(context, http.Status.OK, view(state.greeting))

```

このhandlerは`Hello, Nagi!`を`Result[Response, Error]`として返します。0.2.0では応答予算とrate予算を明示し、全routeへrate policyを設定してから起動します。未検証の値をdefaultとして流用しないでください。詳細は[SF07の予算と起動設定](http-server.md#sf07の明示予算とrate-policy)を参照してください。

## リクエストと応答

| 操作 | 書き方 |
|---|---|
| GETか調べる | `request.is_get` または `request.method == http.Method.GET` |
| パスを読む | `request.path` |
| bodyを借りる | `request.body` |
| ヘッダーを読む | `http.header_text(view(request), "Authorization")` |
| 文字列を返す | `http.text(context, http.Status.OK, "hello")` |
| HTML記法を文字列として返す | `http.text(context, http.Status.OK, "<h1>Hello</h1>")`。これはHTMLとして解釈されません |
| typed HTML文書を返す | `http.html_response(context, http.Status.OK, document)` |
| JSONを返す | `http.json[User](context, http.Status.CREATED, user)` |
| bodyのない応答 | `http.empty(context, http.Status.NO_CONTENT)` |

この表の`context`は現在の`request`から`http.response_context(view(request))`で借ります。応答factoryはすべて`Result[http.Response, Error]`を返します。handlerから直接返すか`try`で伝播し、二重に`ok(...)`で包みません。factoryにstatus-first／contextなしの互換入口はありません。

ヘッダーは存在しないこともあるため、`Result[Option[view[str]], Error]`を返します。`match`の`Ok`／`Err`と`Some`／`None`で分岐します。POSTも`http.Method.POST`で登録できます。

## エラー処理を変える

`http.app[State, AuthError](state, auth_error)`で、`fn(view[http.ResponseContext], AuthError) -> Result[http.Response, Error]`型のmapperを指定できます。`http.route_mapped`も同じ型のroute専用mapperを受け取ります。mapperはrequest-borrowed contextを使って応答を作り、正常な応答は変換しません。応答factoryのResultは直接返すか`try`で伝播します。

[認証サンプル](../test-nagi-code/library-examples/http-auth/README.md)には、Authorizationの読み取り、401とWWW-Authenticate、routeごとの403、型付き状態を含む実行例があります。

## 次に読む

- [HTTP APIリファレンス](http-server.md)：Status、ヘッダー、route、制限の設定
- [標準HTTPの測定結果](http-stdlib-performance.md)：応答速度、メモリ、連続負荷
- [JSON](json.md)：bodyをclassへ変換する
- [廃止したHTTP API](http-legacy.md)：旧decorator／serveの移行

現在の標準サーバーはloopbackのHTTP/1.1に対応します。TLSや外部公開にはリバースプロキシを使います。接続元IPの取得、ストリーミング、WebSocket、HTTP/2の公開APIはありません。外部APIへの送信には[HTTP client](http-client.md)の固定操作APIを使えます。任意URLをrequestから渡すAPIはありません。

内部ではHyperがHTTP通信、Tokioが非同期実行を担います。Nagiはroute登録、型付き状態、Response、制限とエラー処理のAPIを提供します。Axum／Towerへの変更は比較検討中で、現行サーバーの置き換えや性能改善が決まったわけではありません。
