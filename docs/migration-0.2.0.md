# 0.2.0 SF01/SF05への移行（未リリース）

このページは開発sourceのSF01/SF05差分です。Security Foundation全体や正式0.2.0は未リリースです。0.1.xのバイナリで新Policyを利用できるとは扱いません。変更理由は、旧HTTP/issuerの併存によるpolicy省略・無期限proofの維持を避けることです。

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
| 任意Host／未設定Optionsでの起動 | 起動前に`try http.authority(options, https_origin, wire_list, count_limit, byte_limit)`。未設定・重複canonical・不正設定は起動Err。設定とproxy ACLの再上書きもErr | parser/config、全route前拒否と実socket/peerのHost slice。全Cargo/4 OS・TLS/browser受入は別 |
| HTTP/1.0・absolute/authority-form・`*`・Forwarded解釈 | HTTP/1.1 origin-formへ。省略portと明示portを別登録し、proxyはForwarded系を除去し`trusted_proxy`へ実peer IPを登録。runtime拒否400、1.0は505。404/405より先 | 有限通常requestのHost/port/IPv6/重複/peer負例と次の正常requestの容量復帰 |

Host変更は未リリースSF03 sliceです。旧任意Host、末尾dot、特殊port表記、proxy内部Hostは明示登録/書換えへ移行します。設定setterはmove＋Resultで、先のOptionsを失敗時に復活させません。新APIは既resource registry/checked facts/sealed emissionを通り、High・保存Low・手書きLowで同じ契約を使います。

HTTPS originとbackend Hostは別の起動情報です。`serve`の平文loopback HTTPだけでbrowser認証を完成と呼びません。ローカルTLS frontendならorigin `https://localhost:8443`、wire `localhost:8080`、実peer `127.0.0.1`を設定し、証明書のtrust・Forwarded除去・HTTP/1.1終端を配置側で確認します。Secure Cookie例外やHost一致によるCSRF免除はありません。Origin/CSRF/pre-login本体はこの変更の保証外です。

handlerは常に`async (http.Request,shared[S],A)->Result[http.Response,E]`です。旧自動body/query/path抽出はhandlerで明示的にparse/decodeし、JSON応答を`http.json`へ変換します。既存CRUD/Resultの業務動作は移行後のnativeテストへ対応付けています。ルート登録はfallibleです。重複method/pathや曖昧なcapture patternはruntime登録Errになり、起動しません。動的pathの既存標準APIを維持し、旧decorator専用の静的path検査を新しい文字列推論仕様へ置き換えません。

AuthScope/Grantの所有Option/Resultと同task async呼出しは維持します。別Task/Actorへのowned delegationは破壊的変更でSameTask違反です。長い仕事はrequestと独立した業務commandを設計し、request proofをqueueへ保持して後で使うことはできません。容量待機後の保護operationは失効と同じgateでpermitを発行します。発行済みcommandの取消・rollbackは保証しません。

[最小HTTP](http.md) · [認証・認可と失敗表](security.md) · [API reference](http-server.md) · [移行テスト対応表](internal/security-foundation/sf01-compiler-migration-map.md)

move/Task/spawnの意味論、SQLite Pool/Txの取得予算・close・actual join・Outcomeを変更しません。旧Db/dynamic SQLの一本化は独立SF05で行います。custom Rust/Axum hostは標準dispatcherの保証外のtrusted境界として残す理由を明記します。旧standard入口をそこへ自動fallbackする互換層はありません。

## SQLite SF05

| 旧入口 | checker/Rust APIと移行 | 同等業務の検証 |
|---|---|---|
| Db、db_open/exec/all/query/insert/update/write | SF05 migration。runtime public Db/Sql削除。explicit Options、Pool/Tx、literal Query、Parametersへ | security_sf05のHigh/保存Low/手書きLow元位置、native NULL/owned/manual row |
| query/all/execへstring/view | 直接literalの `sqlite.literal("...")` が返すQueryを渡す | Query保存/選択/関数返却とnative bind |
| constructorへ変数・連結・format・動的str | 引数の元位置で拒否。値はbind、構造はreview済み有限Queryから選ぶ | alias/直接import/同名ユーザー関数の拒否・positive |
| INSERT/UPDATE RETURNING、複数文exec | 同Txのexec＋readonly query。匿名?のSQL出現順へParametersを並べる。bootstrapはtrusted固定管理処理へ | CRUD/inventory/task/device-settings/result API native |
| protected SQL | 実Grant subject/targetをowner/tenant predicateへbind。予約後Grant.submitで実queueへ同期enqueue | 別tenant/target0、失効先行enqueue0、受理先行の実SQL、capacity待ち中失効 |

通常checkへSQL engineを必須にしません。opt-in SQLは直接QueryとParameters builder列をprepare-onlyで検査し、不明構造と実値型/NULLはruntimeへ残します。FromRowとRust固定SQLはtrusted adapter境界で、request用動的factoryを再exportしません。Session世代の同Tx検証はSF02の後続です。汎用queryへGrantを足すだけのtenant保証はありません。

[SQLite](sqlite-pool.md) · [保護操作](security.md#保護sqlite操作) · [SF05契約](internal/security-foundation/sf05-contract.md)
