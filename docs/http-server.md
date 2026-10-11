# HTTP APIリファレンス

未リリース0.2.0 SF01のAPIです。[移行](migration-0.2.0.md)と[認証・認可](security.md)も参照してください。

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
| `text(status, text)` | `Response` |
| `bytes(status, body)` | `Response` |
| `json[T](status, value)` | `Result[Response, Error]` |
| `append_header(response, name, value: view[bytes])` | `Result[Response, Error]` |
| `append_header_text(response, name, value: view[str])` | `Result[Response, Error]` |

`text`、`bytes`は借りた入力をコピーして、Responseが所有するbodyを作ります。`json`は値を借用してJSONを作るため、元の値をmoveしません。文字列リテラルはview引数へ直接渡せます。変数から借りる場合は`view(value)`を使います。

ヘッダー追加は応答を受け取り、新しい応答を返します。非reservedの同名ヘッダー（X-Traceなど）は重複を保持します。Content-Length/Transfer-Encoding/Content-Type/Set-Cookie/WWW-Authenticate/Cache-Control/Vary、CORS/CSP/nosniff等のsecurity-managedヘッダーは拒否します。Cookie/Session発行はSF02まで未実装です。HEADではbodyを送らず、GET相当の長さを保持します。204・205・304ではbodyを送信しません。authority-form CONNECTは起動設定節のgateで400になります。origin-form CONNECTのhandlerが成功応答を返した場合もtunnelは未対応で、501で接続を閉じます。

## Appとroute

```nagi
app = http.app[State, AuthError](state, map_error)
app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), handle)
app = try http.route_mapped(app, http.Method.POST, "/login", http.public_policy[State](), login, map_login_error)
limits = try http.authority(http.default_options(), "https://localhost", ["localhost:8080", "127.0.0.1:8080"], 2, 256)
return await http.serve(app, 8080, limits)
```

- `app[S, E](state, mapper)`は状態を所有し、`fn(E) -> Response`を既定のエラー処理にします。
- `app_default[S](state)`は`Error`用の既定処理を使います。
- Policyは`public_policy[S]()`、`authenticated_policy[S](verifier)`、`authorized_policy[S,P](verifier, authorizer)`から作ります。Aは順にunit、AuthScope、Grant[P]です。旧arityにはmigration診断が出ます。
- handlerは`async def handle(request: Request, state: shared[S], access: A) -> Result[Response, E]`です。状態そのものはリクエストごとにコピーしません。Rust側では状態に`Send + Sync`、handlerのFutureに`Send`などが必要で、最終的な適合はbuildで検証します。
- `route_mapped`はhandlerの独自エラー型と、それに合うmapperを指定できます。
- 登録済みGETにHEADを自動で対応させます。HEADの明示登録を優先します。既存パスでmethodが違う場合は405とAllowを返します。

mapperはアプリのエラーを処理します。不正なHTTP、制限超過、タイムアウトはサーバー側で処理します。データベースのエラー詳細や認証情報は応答へ直接出さないでください。

Nagi 0.1.10以降では、応答開始前のhandlerやmapperでunwindするpanicが起きた場合、詳細を含まない500を返し、その接続を閉じます。これはpanicの検出であり、共有状態やDBの更新を巻き戻す仕組みではありません。通常の失敗はResultで返してください。`panic=abort`、OOMなどによるプロセス終了、unwind中の二重panic、独自Rustの解放処理、応答開始後の障害は回復を保証しません。

エラー応答にリクエストIDなどが必要なら、handlerで検証した値を保持し、失敗時に独自エラー型へmoveできます。[見積APIの例](../test-nagi-code/application-examples/quote-api/README.md)では、この方法で共通mapperへIDを渡しています。

## 制限と停止

0.2.0開発sourceでは、`serve`とRustの`serve_listener`にchecked authority設定が必要です。未設定の`default_options()`/`options(...)`だけでは起動Errになります。

| 起動設定 | 契約 |
|---|---|
| `authority(options, external_origin, authorities: List[str], entry_limit, byte_limit)` | HTTPS originと受理wire authorityの有限集合を検査し、`Result[Options, Error]`を返す。件数・bytesは正の有限値。空集合・canonical重複・不正origin/authority・上限超過はErr |
| `trusted_proxy(options, peer_ips: List[str])` | authority設定後だけ、同じ有限件数・bytes上限の実TCP peer IP ACLを設定。空・不正・unspecified/multicast・canonical重複はErr。CIDR/DNSではなく正確IP集合 |

originは`https://host[:port]`のみで、path/userinfo/query/fragmentは認めません。wire側のport省略と明示`:443`等は別entryです。DNSはASCIIの大小文字を正規化し、IPは標準IP parserを使います。IPv6はbracketが必要です。標準IPv4として解釈できない数字とdotだけのhostも拒否します。portはcanonical十進の1〜65535で、leading zero、userinfo、`%`/zone、末尾dot、Unicode、comma listを拒否します。IDNAは事前canonical ASCII A-labelを設定し、runtimeでUnicode変換やIDNA同値性の保証はしません。proxy peerだけは標準IP型でIPv4-mapped IPv6をIPv4へ正規化し、重複を拒否します。

authorityの再設定やproxy ACLの上書きはErrです。setterはOptionsをmoveし、成功Resultだけを起動へ渡します。失敗後に先の設定へ自動fallbackしません。

全route・404/405・body受信・verifier・proof発行より前に、HTTP/1.1のorigin-formと一個の非空Hostを検査します。不正・欠落・重複・非allowlist、absolute-form/authority-form/`*`、ForwardedまたはX-Forwarded-*のpresenceは400、HTTP/1.0は505です。これらruntime拒否はno-storeとcloseです。Hyper自身がservice前に拒否する不正HTTP構文・HTTP/2 prefaceは公式parserへ委譲し、handler/proofには届きません。標準listenerはHTTP/1のみです。

direct/proxyのprofileは起動時に固定します。proxyでもForwarded系はbackendへ送る前に除去し、Hostは登録済み内部値または登録済みpass-through値にします。ACLは`accept`が返す実peerだけで判定し、headerからtrustやoriginを選びません。loopback ACLは同hostの別processを認証しません。Host一致はOrigin/CSRF/CORSを免除せず、それらのSF03機能はこのHost sliceでは未実装です。

`serve`はloopbackの平文HTTPで、origin設定はTLSを実装しません。ローカルのpublic応答確認とbrowser認証を区別してください。browser用は信頼された証明書のTLS frontendを置き、たとえばexternal origin `https://localhost:8443`、backend wire `localhost:8080`、peer `127.0.0.1`を明示します。HTTP/2 frontendもHTTP/1.1へ終端します。Secure Cookieにlocalhost例外はありません。実TLS/browser・4 OSの検証は別の受入条件です。

| 設定 | 既定値 | 変更する関数 |
|---|---|---|
| bodyの上限 | 1 MiB | `options(body_bytes, body_ms, handler_ms, shutdown_ms)` |
| bodyの受信期限 | 10秒 | `options(...)` |
| handlerの期限 | 2秒 | `options(...)` |
| verifier＋authorizerの共通期限 | 2秒 | `security_timeout(options, milliseconds)` |
| 停止時の待ち時間 | 10秒 | `options(...)` |
| 接続数／同時リクエスト | 1024／256 | `capacity(options, connections, requests)` |
| ヘッダー・次のリクエスト待ち | 10秒 | `header_timeout(options, milliseconds)` |
| 応答の送信期限 | 10秒 | `send_timeout(options, milliseconds)` |
| ヘッダーバッファ／件数 | 32 KiB／100件 | `header_limits(options, bytes, count)` |

securityの期限はverifierとauthorizerを合わせた一つの絶対予算です。`security_timeout`は正のミリ秒のみ受理し、0や負数で無期限にはできません。認証後もbody・handlerの独立した期限は維持します。認証情報の期限が先に切れた場合、handlerへ渡す前に再検査します。

変更する関数はすべて`Result[Options, Error]`を返します。接続数と同時リクエスト数は受け付けの上限で、スレッド数ではありません。Ctrl+Cでは新しい接続を止め、処理中の接続を待ちます。停止期限後には接続taskを中止して回収します。実行開始済みのblocking処理は強制停止できません。

Nagi 0.1.10以降では、handlerの応答生成が正常に完了した時点で期限を過ぎていれば、504を返します。同期処理を期限時刻に強制停止する機能ではありません。処理が制御を戻すまで応答は遅れ、すでに行った状態変更も巻き戻しません。

本文上限を超えると、handlerを呼ばずに413と`Connection: close`で拒否します。0.2.0開発版では応答の送信後、最大8 KiB・100 msと本文期限の短い方まで残りの入力を読み捨て、書込み側を閉じます。この間は接続枠を保持し、読み捨てた入力をhandlerへ渡しません。通常応答やidle Keep-Aliveではこの後片付けを行いません。残りの本文を最後まで読み捨てる実装ではなく、期限・上限・取消で切断します。未読本文を残した切断ではTCP resetが起こり得るため、すべてのOS・client・送信条件で413を受信できることは保証していません。

設定例：

```nagi
limits = try http.options(1048576, 10000, 2000, 10000)
limits = try http.capacity(limits, 2048, 512)
limits = try http.header_limits(limits, 32768, 100)
limits = try http.authority(limits, "https://localhost", ["localhost:8080", "127.0.0.1:8080"], 2, 256)
return await http.serve(app, 8080, limits)
```

## 実装の対応

NagiのAPIは[標準moduleの登録](../compiler/src/stdlib.rs)と[HTTP runtime](../runtime/src/http_server.rs)で実装しています。通信はHyperのHTTP/1、実行と接続taskの管理はTokio、pathの照合はmatchit、JSON変換はSerdeです。AxumのHTTP型も使いますが、このAPIのroute処理はAxum Routerではありません。

[HTTP runtimeのテスト](../runtime/src/http_server/tests.rs)には、実際のsocketを使う応答、制限、タイムアウト、接続終了の確認があります。[Rust連携](modules-and-rust.md)では独自のサーバーを作れますが、同じ制限や障害処理が自動で付くわけではありません。
