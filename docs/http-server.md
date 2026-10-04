# HTTP APIリファレンス

```nagi
import std.http.server as http
from std.http.server import Status as Code
```

## MethodとStatus

`http.Method.GET`、`POST`、`PUT`、`DELETE`、`PATCH`、`HEAD`、`OPTIONS`、`CONNECT`、`TRACE`を使えます。`request.method`との比較は借用し、Methodをコピーしません。`request.is_get`や`request.is_post`も使えます。`http.method("PROPFIND")`は拡張methodを検証して作ります。

Statusは数値を保持するコピー可能な値です。文字列への変換は比較や応答のために必要ありません。

| 用途 | 例 |
|---|---|
| 名前で指定 | `Code.OK`、`CREATED`、`NO_CONTENT`、`UNAUTHORIZED`、`FORBIDDEN`、`CONFLICT`、`TOO_MANY_REQUESTS` |
| 数値から作る | `try http.status(418)`。最終応答として200〜599を検証する |
| 数値を読む | `Code.NOT_FOUND.value` → `404` |
| 説明を読む | `Code.NOT_FOUND.phrase` → `"Not Found"` |
| 分類する | `.is_success`、`.is_redirection`、`.is_client_error`、`.is_server_error` |

標準の2xx〜5xx定数を用意しています。`.phrase`は静的な文字列参照で、未登録の数値では空文字列です。1xxや101のprotocol切り替えは通常の最終応答には使えません。

## Request

| フィールド | 型 |
|---|---|
| `method` | `http.Method` |
| `path` | `view[str]` |
| `query` | `Option[view[str]]`。`?`を除いた未変換のquery |
| `body` | `view[bytes]` |
| `is_get`、`is_post`など | `bool` |

`path`、`query`、`body`とヘッダーはRequest内のデータを借ります。参照を使用中にRequestをmoveできません。path登録は`"/users/{id}"`のような形にも対応します。captureの型付き取り出しAPIと接続元IPの取得APIはありません。本文は上限まで読み込んでからhandlerへ渡すため、受信ストリームではありません。

| 関数 | 戻り値 |
|---|---|
| `header(view(request), name)` | `Result[Option[view[bytes]], Error]` |
| `header_text(view(request), name)` | `Result[Option[view[str]], Error]` |
| `headers(view(request), name)` | `Result[List[view[bytes]], Error]` |
| `is_json_content_type(view(request))` | `Result[bool, Error]` |
| `method_name(view(request.method))` | `view[str]` |

ヘッダー名は大文字・小文字を区別しません。単一取得で同名ヘッダーが複数ある場合はエラーです。`headers`はすべて返します。`header_text`はUTF-8を検証します。

`is_json_content_type`は`application/json`を大文字・小文字を区別せずに判定し、前後の空白・タブと`charset=utf-8`などのparameterを受け付けます。欠落や別のmedia typeは`False`、重複ヘッダーや不正な構文は`Err`です。`application/problem+json`は別のmedia typeです。parameterは構文だけを検証し、`charset`で文字コードを切り替えません。JSON本文はUTF-8で読みます。この判定はヘッダーを借り、本文をコピーせず、成功時にメモリを確保しません。

## Response

| 関数 | 戻り値 |
|---|---|
| `empty(status)` | `Response` |
| `text(status, text)`、`html(status, text)` | `Response` |
| `bytes(status, body)` | `Response` |
| `json[T](status, value)` | `Result[Response, Error]` |
| `append_header(response, name, value: view[bytes])` | `Result[Response, Error]` |
| `append_header_text(response, name, value: view[str])` | `Result[Response, Error]` |

`text`、`html`、`bytes`は借りた入力をコピーして、Responseが所有するbodyを作ります。`json`は値を借用してJSONを作るため、元の値をmoveしません。文字列リテラルはview引数へ直接渡せます。変数から借りる場合は`view(value)`を使います。

ヘッダー追加は応答を受け取り、新しい応答を返します。Set-Cookieなどの重複を保持し、不正な名前・改行・Content-Length／Transfer-Encodingの指定は拒否します。HEADではbodyを送らず、GET相当の長さを保持します。204・205・304ではbodyを送信しません。成功CONNECTのtunnelは未対応で、501で接続を閉じます。

## Appとroute

```nagi
app = http.app[State, AuthError](state, map_error)
app = try http.route(app, http.Method.GET, "/", handle)
app = try http.route_mapped(app, http.Method.POST, "/login", login, map_login_error)
return await http.serve(app, 8080, http.default_options())
```

- `app[S, E](state, mapper)`は状態を所有し、`fn(E) -> Response`を既定のエラー処理にします。
- `app_default[S](state)`は`Error`用の既定処理を使います。
- handlerは`async def handle(request: Request, state: shared[S]) -> Result[Response, E]`です。状態そのものはリクエストごとにコピーしません。Rust側では状態に`Send + Sync`、handlerのFutureに`Send`などが必要で、最終的な適合はbuildで検証します。
- `route_mapped`はhandlerの独自エラー型と、それに合うmapperを指定できます。
- 登録済みGETにHEADを自動で対応させます。HEADの明示登録を優先します。既存パスでmethodが違う場合は405とAllowを返します。

mapperはアプリのエラーを処理します。不正なHTTP、制限超過、タイムアウトはサーバー側で処理します。データベースのエラー詳細や認証情報は応答へ直接出さないでください。

mainでは、応答開始前のhandlerやmapperでunwindするpanicが起きた場合、詳細を含まない500を返し、その接続を閉じます。公開0.1.9にはこの修正が入っていません。これはpanicの検出であり、共有状態やDBの更新を巻き戻す仕組みではありません。通常の失敗はResultで返してください。`panic=abort`、OOMなどによるプロセス終了、unwind中の二重panic、独自Rustの解放処理、応答開始後の障害は回復を保証しません。

エラー応答にリクエストIDなどが必要なら、handlerで検証した値を保持し、失敗時に独自エラー型へmoveできます。[見積APIの例](../test-nagi-code/application-examples/quote-api/README.md)では、この方法で共通mapperへIDを渡しています。

## 制限と停止

| 設定 | 既定値 | 変更する関数 |
|---|---|---|
| bodyの上限 | 1 MiB | `options(body_bytes, body_ms, handler_ms, shutdown_ms)` |
| bodyの受信期限 | 10秒 | `options(...)` |
| handlerの期限 | 2秒 | `options(...)` |
| 停止時の待ち時間 | 10秒 | `options(...)` |
| 接続数／同時リクエスト | 1024／256 | `capacity(options, connections, requests)` |
| ヘッダー・次のリクエスト待ち | 10秒 | `header_timeout(options, milliseconds)` |
| 応答の送信期限 | 10秒 | `send_timeout(options, milliseconds)` |
| ヘッダーバッファ／件数 | 32 KiB／100件 | `header_limits(options, bytes, count)` |

変更する関数はすべて`Result[Options, Error]`を返します。接続数と同時リクエスト数は受け付けの上限で、スレッド数ではありません。Ctrl+Cでは新しい接続を止め、処理中の接続を待ちます。停止期限後には接続taskを中止して回収します。実行開始済みのblocking処理は強制停止できません。

本文上限を超えると、handlerを呼ばずに413と`Connection: close`で拒否します。残りの本文を最後まで読み捨てる実装ではありません。未読本文を残した切断ではTCP resetが起こり得るため、すべてのOS・client・送信条件で413を受信できることは保証していません。

設定例：

```nagi
limits = try http.options(1048576, 10000, 2000, 10000)
limits = try http.capacity(limits, 2048, 512)
limits = try http.header_limits(limits, 32768, 100)
return await http.serve(app, 8080, limits)
```

## 実装の対応

NagiのAPIは[標準moduleの登録](../compiler/src/stdlib.rs)と[HTTP runtime](../runtime/src/http_server.rs)で実装しています。通信はHyperのHTTP/1、実行と接続taskの管理はTokio、pathの照合はmatchit、JSON変換はSerdeです。AxumのHTTP型も使いますが、このAPIのroute処理はAxum Routerではありません。

[HTTP runtimeのテスト](../runtime/src/http_server/tests.rs)には、実際のsocketを使う応答、制限、タイムアウト、接続終了の確認があります。[Rust連携](modules-and-rust.md)では独自のサーバーを作れますが、同じ制限や障害処理が自動で付くわけではありません。
