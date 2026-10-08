# SF01: 公開APIと境界契約

[RFC](rfc.md)・[確定判断/移行](decisions-and-migration.md)を基準とする。SF00のhead `a97c7fbd1cd32a40806e21e0d9a6f07a04073dbd`はchecks37753183787/37753178468・website37753183429・merge gate37753180262成功、独立SF-R01–06再確認済み。PR #100はユーザー承認でmainへマージ済み（10655ea7299d775235ab6585e5ab8e321f121593、treeはa97と同一）。SF01のPR #101はmain向けDraft。以下は実装対象の契約で、GREEN/native/4 OSの証拠が揃うまで完成と記載しない。

## 公開API

標準型はcanonical IDから解決し、同名のユーザー型/関数を占有しない。

- `auth.AuthScope`: opaque、verified subject/request ID/private lease/絶対期限。SF01のcredential sourceは一意のBearer headerだけ。非Copy/Clone/Serde/shared/field、SameTask。`auth.subject(view(scope))`はIDの読取のみ。
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

## 独立境界レビューと追加RED

Sol High独立レビューは失効/permit発行の同Mutex、Policy protocol metadata、SameTask、最終Low/native sealを確認した。旧RoutePlan/needs_server/__route__/__nagi_serveはmigration拒否されるdecorator/global builtinだけの生成経路であり、標準routeの動的登録検査とは別である。main引数禁止と旧decoratorの元位置診断は維持して生成経路を削除した。

削除操作は初回の自動承認レビューで「新routeの競合検査を失う」と拒否された。ソース照合・独立レビュー・移行後High/保存Lowの実登録Errを追加し、同じ削除を根拠付きで再実行して承認された。拒否を別経路で迂回していない。

旧decoratorの静的path競合は、標準Appのfallible dynamic登録ではmatchitの登録Errになる。pathを変数/分岐で作る既存標準APIを維持し、checkerに文字列/名前推測による別routing仕様を追加しない。曖昧routerを起動しない同等業務をnativeで確認する。

レビュー候補だったnon-yielding verifierの期限後Errについて、小さい20ms/5ms fixtureで403対504のREDを保存した。共通deadline判定をOk/Err/panic全結果へ適用した。同期処理の強制停止や副作用rollbackは保証しない。Bearer security401ではWWW-Authenticateをframeworkが所有し、Failureの安定messageを返す。アプリの通常Result Errはmapper、security Failureはhandlerに入る前の固定分類で分離する。

## 最終独立レビューで見つかった実装欠陥

- P1: Policy/VerifiedIdentity/Failureをnested copyしてcheckを通過するとforeign型Clone不足でRustが拒否する。コピーの作業対象をたどる用途別能力検査へ修正。Copy boolをCloneと同一視せず、所有wrapper/record fieldだけをたどり、Arc handle・関数署名・native protocol/phantomを複製対象としない。利用者classの手書きCloneは従来どおりrustc責務。
- P2: Policyのwrapper/field/shared型注釈とApp shared state経由が宣言したnonsharedと不一致。認証proof検査へPolicyを混ぜず、既知の新nonshared資源をphysical retention境界で別検査する。
- P1: TaskのHTTP native oracleに旧Rust route arityが5箇所残った。public Policyとunit handler引数へ移行し、業務Err・terminal故障・actual shutdown/parent Dropの元assertionsを維持する。

copy/shareは先行REDを保存後、High/保存Lowのcheck診断＋両元行、手書きLowのcheck/build（PATH空でCargoへ進まない）、native @replaceを追加した。fn-pointer出力をClone payloadと誤扱いしない正例も3経路nativeで検証。最終独立再確認と最新source headのCIを結果資料へ別記録する。
