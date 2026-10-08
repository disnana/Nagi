# 0.2.0 SF01への移行（未リリース）

このページは開発sourceのSF01差分です。Security Foundation全体や正式0.2.0は未リリースです。0.1.xのバイナリで新Policyを利用できるとは扱いません。変更理由は、旧HTTP/issuerの併存によるpolicy省略・無期限proofの維持を避けることです。

| 旧API/動作 | checker診断と移行 | 同等業務の検証 |
|---|---|---|
| `@get/@post/@put/@delete` | `SF01 migration`。Appへmethod/path/明示Policy/handlerを登録 | `http_entrypoint`、`http_route_conflicts`、`error_routes`、Python route/CRUD integration |
| `serve(Db, port)` | `SF01 migration`。`http.serve(app,port,options)`。DBなしでも起動可能 | High/保存Low nativeの起動・不正portとエラー処理 |
| 旧4引数route/5引数route_mapped | `SF01 migration`。handlerの前へPolicyを追加し、第3引数をunit/AuthScope/Grantへ | 既存HTTP checker/native、security_sf01_nativeの3経路 |
| `Principal`、生subjectのissuer、unchecked parts | `SF01 migration`/Rust API削除。verifier→dispatcher AuthScope→authorizer Grant→native submit | 認証・認可サンプル、lease失効、失効/permit順序、marker/対象一回consume |
| 組み込み`/health` | 明示public routeへ登録。未登録は404 | native High/Low・socket |
| 組み込み`/stream`・`/ws` | 暗黙注入を廃止。有限bytes応答は明示route。一般streaming/WSは未実装 | 暗黙routeがない検査。WS同等業務は未完了 |
| raw `html`/`Html`/`http.html` | 標準入口を拒否。文字列表示はtext、active HTMLはSF04のtyped移行を待つ | raw入口拒否、octet-stream/text・nosniff。HTML画面の同等移行は未完了 |
| 任意Set-Cookie/CORS/CSP/cache/challenge/framing header | appendでInvalid。専用の後続security層が所有する | managed-header/wire回帰。Cookie/CSRF/CORS機能は未完了 |

handlerは常に`async (http.Request,shared[S],A)->Result[http.Response,E]`です。旧自動body/query/path抽出はhandlerで明示的にparse/decodeし、JSON応答を`http.json`へ変換します。既存CRUD/Resultの業務動作は移行後のnativeテストへ対応付けています。ルート登録はfallibleです。重複method/pathや曖昧なcapture patternはruntime登録Errになり、起動しません。動的pathの既存標準APIを維持し、旧decorator専用の静的path検査を新しい文字列推論仕様へ置き換えません。

AuthScope/Grantの所有Option/Resultと同task async呼出しは維持します。別Task/Actorへのowned delegationは破壊的変更でSameTask違反です。長い仕事はrequestと独立した業務commandを設計し、request proofをqueueへ保持して後で使うことはできません。容量待機後の保護operationは失効と同じgateでpermitを発行します。発行済みcommandの取消・rollbackは保証しません。

[最小HTTP](http.md) · [認証・認可と失敗表](security.md) · [API reference](http-server.md) · [移行テスト対応表](internal/security-foundation/sf01-compiler-migration-map.md)

move/Task/spawnの意味論、SQLite Pool/Txの取得予算・close・actual join・Outcomeを変更しません。旧Db/dynamic SQLの一本化はSF05で、SF01へ混ぜていません。custom Rust/Axum hostは標準dispatcherの保証外のtrusted境界として残す理由を明記します。旧standard入口をそこへ自動fallbackする互換層はありません。
