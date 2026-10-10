# 認証・認可（未リリース0.2.0 SF01）

認証は「誰か」を確認し、認可は「この対象へこの操作をしてよいか」を確認します。SF01は標準HTTPの明示Policy、requestに結び付いたproofと終了時の失効を提供します。永続Session・Cookie発行・CSRF/CORS・暗号verifier・typed HTML・送信HTTPは後続工程で、Security Foundation全体が完成したとは扱いません。公開済み0.1.xへ遡及適用しません。

## すべてのrouteで方針を選ぶ

| Policy | handlerの第3引数 | 用途 |
|---|---|---|
| `http.public_policy[S]()` | `unit` | 認証不要で公開するroute。proofを発行しない |
| `http.authenticated_policy[S](verify)` | `auth.AuthScope` | 有限期限のverified subjectを必要とするroute |
| `http.authorized_policy[S,P](verify, authorize)` | `auth.Grant[P]` | nominal permission Pと実対象i64を必要とするroute |

```nagi
import std.http.server as http
class State:
    greeting: str
async def hello(request: http.Request, state: shared[State], access: unit) -> Result[http.Response, Error]:
    return ok(http.text(http.Status.OK, view(state.greeting)))
async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(greeting="Hello, Nagi!"))
    app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), hello)
    return await http.serve(app, 8080, http.default_options())
```

開発compilerで作業フォルダーの`server.nagi`へ保存し、同じ場所で`nagic run server.nagi`を実行します。別ターミナルで`curl http://127.0.0.1:8080/`を実行すると`Hello, Nagi!`です。終了はCtrl+C。port使用中なら別portを選び、compiler導入は[準備](getting-started.md)、制限と診断は[HTTP reference](http-server.md)を確認してください。

Policyは非Copy・非sharedの設定値です。routeへmoveし、再利用する場合は新しいPolicyを作ります。型検査はPolicyのState・出力型とhandlerの3引数を照合します。publicを選ぶとproofを要求するhandlerは接続できません。明示publicの選択が業務上正しいか、verifier/authorizerの内容が正しいかまでは証明しません。

Policy・VerifiedIdentity・auth.Failureは既知の非Clone・非shared資源です。Option/List/classに包んでもcopyや共有はできません。関数ポインターをコピーする操作は、その戻り値を複製する操作ではありません。Copy値の代入と、明示copyが使うClone能力は区別します。利用者classの手書きRust Cloneの適合は従来どおり最終buildで確認します。

## verifierとauthorizer

verifierは名前付きasync関数またはそのローカルaliasで、`(http.Request, shared[S]) -> Result[auth.VerifiedIdentity, auth.Failure]`です。authorizerは`(auth.AuthScope, http.Request, shared[S]) -> Result[auth.Grant[P], auth.Failure]`です。両者が見るRequestは有限のmethod/path/query/headerを持ち、bodyは空です。handlerはbody制限後に元のRequestを受け取ります。

SF01の認証sourceは一つのAuthorization Bearer headerだけです。欠落・不正credentialは401、重複headerやCookieとの混在は400でhandlerを呼びません。public policyは認証を行いません。login/session/browser運用を固定credentialデモで代用しないでください。

trusted Rust verifierが暗号・audience・expiry・revocationを確認してから`VerifiedIdentity::from_verified(subject, absolute_expiry)`を返します。これはproofではなく、Nagiのfactoryはありません。dispatcherだけがrequest leaseを発行しAuthScopeへ結び付けます。trusted authorizerは実対象とPのpolicyを確認してから`Grant::from_authorized(scope, target)`を使います。[完全な認可サンプル](../test-nagi-code/application-examples/auth-boundary/README.md)を参照してください。固定credentialは小さい検証入力で、production verifierではありません。

## proofの移動と有効期間

AuthScopeとGrantはopaqueで、構築・Copy/Clone・JSON復元・shared・class/enum field格納を拒否します。同じtaskの所有引数・return・local Option/Resultとasync delegationは使えます。別Taskのcapture/結果、Actor、proofを捕捉したFutureのtransferはSameTask違反です。`move`は所有者を移すだけで、期限を延長しません。`auth.subject(view(scope))`のi64はIDでありproofではありません。

leaseの期限は有限security/body/handler予算とverified identityの絶対期限の最小値です。正常終了・業務Err・panic・timeout・取消・shutdown・未poll/Pending FutureのDropでdispatcherのownerが失効させます。Rust adapterがproofを保持しても次の保護operationへ使えません。同期Dropは任意native仕事の終了確認やrollbackを意味しません。

保護adapterは先に有限native capacityを取得します。その予約だけでは操作を受理したことになりません。`grant.submit(reservation, callback)`が失効と同じ短いgateで現在時刻と有効状態を確認し、一回限りのprivate execution permitを発行します。callbackはbound subject/target/予約を使って同期enqueueします。gate内でawaitせず、deferred Futureを返すだけでenqueueの代用にしてはいけません。これら任意Rust内部の正しさはtrusted adapterの責務です。permit発行後の失効は受理済みI/Oを取り消しません。

## 失敗・HTTP境界

| auth.FailureKind | HTTP | 安定message |
|---|---|---|
| `INVALID_CREDENTIAL` | 401 + Bearer challenge | `invalid credential` |
| `DENIED` | 403 | `permission denied` |
| `EXPIRED` | 401 + Bearer challenge | `authority expired` |
| `INVALID_REQUEST` | 400 | `invalid security request` |
| `UNAVAILABLE` | 503 | `security service unavailable` |
| `INTERNAL` | 500 | `security service failure` |

`auth.kind(view(failure))`と`auth.message(view(failure))`で読みます。messageのviewはFailureの借用として扱います。security予算の超過は504、unwind panicは詳細を伏せた500です。handlerの業務Result Errは既定/route mapperが扱い、security Failureとflattenしません。

`security_timeout(options, positive_ms)`はverifierとauthorizer共通の絶対予算を設定します。待機ごとにリセットしません。HEAD fallbackはGET policyを使い、明示HEAD/OPTIONSもPolicyが必要です。404/405はproofとhandlerを発行しません。CORS preflightはSF03まで未実装です。

finalizerがContent-Typeとnosniff、reserved security/framing headersを所有します。raw HTML・Set-Cookie・CORS/CSP・任意cache/challengeの追加による迂回は拒否します。panic=abort、OOM、non-yielding処理の強制停止、任意Rustの二重panic、外部副作用のrollbackは保証しません。標準HTTP外のcustom Rust hostは明示したtrusted境界で、このPolicy保証を自動適用しません。

[移行](migration-0.2.0.md) · [move](ownership.md) · [Task](task-handles.md) · [SQLite](sqlite-pool.md) · [採用契約](internal/security-foundation/sf01-contract.md)

## 保護SQLite操作

未リリースSF05は、Queryを直接literalから作り、実値をParametersへbindする標準入口へ統一します。QueryはSQL構造の出所を制限する型です。Grantやtenant制約を内包する型ではありません。

保護された操作はreview済みtrusted adapterに固定SQLと所有者predicateを置き、`Grant.submit` が渡す**実subjectと実target**を `WHERE owner=? AND id=?` などへbindします。任意queryにGrantを追加するだけでは対象の制限を証明できません。保護対象のsession/owner条件が可変なら、同じTxのpredicateまたは同じTxでの再確認が必要です。永続Session・世代管理はSF02の後続であり、SF05の完了保証へ数えません。

trusted Rust hostはTxのopaque `reserve_exec` でbounded queueの容量を待ち、予約後に `Grant.submit` の同期callback内で `reservation.enqueue(Query, Parameters)` を呼びます。容量予約は受理ではありません。admissionはgate内の一回permit発行で順序化します。発行前の失効ならnative enqueueは0、発行後の失効ならcallbackの同期enqueue前でも受理済みで、後の失効やHTTP取消は取り消し・rollbackを保証しません。callbackはFutureを返して後からenqueueする形にせず、同期で実queueへ送ります。

DDL/bootstrapはrequestと分離したtrusted管理処理です。Rust hostの固定SQL factoryや手書きFromRowはreview対象です。動的文字列factoryをrequestへ再exportして標準literal契約を迂回しません。[SQLite](sqlite-pool.md) · [移行](migration-0.2.0.md) · [SF05契約](internal/security-foundation/sf05-contract.md)
