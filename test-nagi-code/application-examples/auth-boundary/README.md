# 標準HTTP policyを通す認証・認可境界

[English](README.en.md)

標準HTTP dispatcherが資格情報を検証して`AuthScope`を作り、Nagiのpolicyが文書へのアクセスを判定し、native adapterが対象に結び付いた`Grant[Read]`を消費してSQLiteを読む例です。認証前のhandlerへリクエストを渡さず、Nagiから`AuthScope`や`Grant`を作ることもできません。

```sh
nagic run --project test-nagi-code/application-examples/auth-boundary
curl -i http://127.0.0.1:8098/health
curl -i -H 'Authorization: Bearer demo-alice' http://127.0.0.1:8098/documents/1
curl -i -H 'Authorization: Bearer demo-alice' http://127.0.0.1:8098/documents/2
```

`/health`は明示したpublic routeです。`/me`、`/documents/{id}`、`POST /documents/read`は`authenticated_policy`を通ります。Aliceは文書1、Bobは文書2を読めます。別の利用者の文書とblocked文書3は403、存在しない文書は404です。JSON入力の構文エラーは400、型エラーは422です。

`verify`は標準dispatcherから`Request`と状態を受け、`VerifiedIdentity`または有限期限付きの`Failure`を返します。`AuthScope`はdispatcherがpolicyに渡します。名前付きNagi `read_policy`（既定）または同じ条件の手書きRust policyが対象を確認した後、`Grant::from_authorized(scope, document_id)`を作ります。adapterはbounded semaphoreの空きをawaitしてから、Grantの対象・subjectを使う同期SQLite commandを実行し、DB内のowner/blocked状態も再確認します。裸のsubjectやIDだけを根拠にしたread pathはありません。

`NAGI_AUTH_POLICY_MODE=nagi`（既定）と`rust`は同じ標準HTTP router、DB、payload、native verifier、Grant消費処理を使い、policy実装だけを切り替えます。`NAGI_AUTH_PROBES=1`を付けると、認証済みpanic/timeout routeも有効になります。本文上限は4096 byte、security/body/handler期限は各1000 ms、HTTP処理上限は32、同時DB read受付は8です。body timeoutは408、handler timeoutは504、panic後の応答は500です。

標準policyの認証失敗はsecretを含まない共通本文を返し、Bearer失敗には`WWW-Authenticate: Bearer`を付けます。資格情報なしと誤った資格情報は同じ401本文`invalid credential`です。期限切れfixtureは`authority expired`、重複または不正なAuthorization headerは400と`invalid security request`になります。Nagi handlerの業務エラー応答は別扱いです。

native verifier内の`demo-alice`、`demo-bob`、`demo-expired`はテスト用credential fixtureです。これらの文字列は組み込みusernameではなく、verifierがsubjectへの対応を決めます。JWT署名、issuer/audience、失効照会、token発行は実装していません。本番では既存の審査済みverifierを接続してください。Nagi/Rust policyの正しさや全routeの設計をcompilerが証明するわけではありません。SQLiteは小さなデモquery用で、汎用pool/transaction APIや取消時rollbackはありません。loopback HTTP/1で動き、TLS、HTTP/2、connection admission制限を含みません。

このサンプルは以前のAxum glueと`Principal` factoryを標準HTTPのrequest-bound auth APIに置き換えています。別例の[custom Axum service](../axum-service/README.md)は、標準HTTP policyを迂回しない独立したtrusted host境界として残っています。
