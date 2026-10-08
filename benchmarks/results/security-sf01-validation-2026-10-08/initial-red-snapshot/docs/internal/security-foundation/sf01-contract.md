# SF01: 公開APIと境界契約

[RFC](rfc.md)・[確定判断/移行](decisions-and-migration.md)を基準とする。SF00のhead `a97c7fbd1cd32a40806e21e0d9a6f07a04073dbd`はchecks37753183787/37753178468・website37753183429・merge gate37753180262成功、独立SF-R01–06再確認済み。PR #100はready・未マージ。SF01 branchはそのheadをbaseとする別Draftで、最終反映先はmain。以下は実装対象の契約で、GREEN/native/4 OSの証拠が揃うまで完成と記載しない。

## 公開API

標準型はcanonical IDから解決し、同名のユーザー型/関数を占有しない。

- `auth.AuthScope`: opaque、verified subject/credential source/request ID/private lease/絶対期限。非Copy/Clone/Serde/shared/field、SameTask。`auth.subject(view(scope))`はIDの読取のみ。
- `auth.Grant[P]`: 同じ制約、nominal permission P、subject/対象実値i64/同じprivate leaseを持つ。一回consume。Rustのtrusted authorizerは `Grant::from_authorized(scope, resource) -> Result[Grant[P], auth.Failure]` を使い、policy成功をその前に確認する。生subjectだけでproofを作るfactoryやunchecked parts取出しは廃止する。
- `auth.VerifiedIdentity`: trusted verifierの期限付き結果。proofではなく、dispatcherだけが生きたrequest leaseへ結び付ける。Rust factoryはsubjectと有限の絶対期限を要求し、Nagi factoryはない。
- `auth.Failure`とCopy enum `auth.FailureKind`: INVALID_CREDENTIAL / DENIED / EXPIRED / INVALID_REQUEST / UNAVAILABLE / INTERNAL。安定した秘密なしmessageをviewで読み、kindを値で読む。denial/expiryは業務Resultで、TaskFailureへflattenしない。panic/unexpected cancellationは既存故障境界。
- `http.Policy[S,A]`: routeへ一回移動するopaque configuration。Aは実proofを格納するpayloadではなくcallback output protocol。factoryの種類からのみ構築し、動的boolやunchecked constructorはない。
- `http.public_policy[S]() -> Policy[S,unit]`。
- `http.authenticated_policy[S](verifier) -> Policy[S,auth.AuthScope]`。verifierは名前付きasync `(http.Request, shared[S]) -> Result[auth.VerifiedIdentity,auth.Failure]`。
- `http.authorized_policy[S,P](verifier, authorizer) -> Policy[S,auth.Grant[P]]`。authorizerは名前付きasync `(auth.AuthScope,http.Request,shared[S]) -> Result[auth.Grant[P],auth.Failure]`。
- `http.route(app, method, path, policy, handler)` / `route_mapped(..., mapper)`。handlerは名前付きasync `(http.Request,shared[S],A) -> Result[http.Response,E]`。PolicyのS/Aとhandler引数をcheckerで一致させ、mapper E/Responseもsealed call factsへ固定する。publicのAはunitで、auth proofは発行しない。
- `http.security_timeout(options, ms)`はpositiveで表現可能な期限のみ受理。security検査にはverifierとauthorizerを合わせた一つの絶対予算を使う。body/handler/send/shutdownの既存独立予算は維持する。

verifier/authorizerへ渡すRequestはmethod/path/query/headerだけの有限snapshotでbodyは空。handlerへ渡す元Requestは既存の有限body受信後のもの。初版の標準credential sourceは明示header-only bearerで、Cookie/session verifierはSF02/03まで利用不可。cookie認証へのfallback、token由来issuer/鍵/URL選択をfactoryへ持ち込まない。publicは意図的匿名で、全route privateという保証ではない。暗黙publicはない。

## 所有権と有効期間

AuthScope/Grantは同taskのowned引数/return・Option/Result localと同task async呼出しを許す。field/shared/Actor/Task結果・captureと、proofを捕捉したFutureのtransferを拒否する。Pの関数署名/phantom型を実payloadとして誤拒否しない。Tx/Taskの既存処理とcanonical SameTask検査を利用し、diagnosticではauthとSQLiteを区別する。

request lease ownerはdispatcherがprivateに保持する。期限はrequestの有限security/body/handler予算とidentityの絶対期限の最小値。await/idle touchで延長しない。正常/Err/mapper/timeout/panic/shutdown/cancel/unpolled Dropにownerが失効させる。Rust hostの故意の偽verifier/policyはtrusted境界で、型の保証外。

native capacity予約はadmissionではない。予約取得後に `Grant::submit(reservation, submit)` が短い局所gate内でactive/現在時刻を検査し、permission/subject/対象/所有reservationへ結び付くprivate一回execution permitを発行する。gate内にawaitはなく、owner失効と同じgateで順序化する。permitを内部でconsumeして、trusted adapterの同期submit callbackへbound subject/対象/reservationを渡す。callbackはその予約へcommandを同期投入する契約で、別bare targetへ置換しない。public permit、reactivate、parts getterを作らない。任意Rust callbackの内部をcheckerが証明するとは説明しない。

予約待ち/事前check後・permit発行前の失効はnative command開始0、reservation Drop。逆順は受理済みで、後の取消で任意副作用をrollbackしない。DB世代更新と局所gateは別の線形化点。同file/同Txのpredicateなしでglobal即時失効を主張しない。SQL/client実adapterへの接続はSF05/SF06の専用oracleも必要。

## HTTPの網羅と移行

全標準HTTP登録は同じpolicy-required dispatcher。HEAD fallbackはGET policy、明示HEAD/OPTIONSもpolicy必須。404/405/early errorはproof/handlerを発行せず統合response finalizerを通す。CORS preflightの専用処理はSF03まで未実装。handler/mapper/policy Future構築とpollのpanicはcatch境界に含め、失効とpermit解放を検証する。既存non-yieldingの強制停止は保証しない。

旧4/5引数route、旧decorator/global serve、Principal型はcheckerのmigration診断で拒否する。alias/reexport/保存Low/手書きLow/native replaceでも同じidentity検査を使う。診断用tombstone以外の実装、runtime旧serve/public factory、組込endpoint注入を残さない。ユーザー定義の同名serve/route/Principalは従来どおりユーザー値。

finalizerがContent-Typeとnosniff、security-managed headersを所有する。append_headerは非reservedのみ。Content-Length/Transfer-Encoding/Content-Type/Set-Cookie/CORS/CSP/安全関連Cache-Control等を任意値で追加して迂回しない。transport early errorsにも同じ基本headersを付ける。Session発行/no-storeの専用所有はSF02、CORS/CSRFはSF03、typed HTMLはSF04で追加する。raw HTML標準入口は診断で拒否し、SF01でXSS renderer完成と主張しない。旧HTML業務例の完全なtyped移行はSF04の残事項として明記する。

旧API拒否をparser/import/未定義エラーの成功に数えず、元位置migration診断と移行後同等なHTTP/認可/CRUD業務nativeを対にする。native proof/leaseのfakeはテスト専用privateで、公開factoryへ混ぜない。REDのstage、診断、元行とnative effectの観測を保存する。
