# RFC: Nagi 0.2.0 Security Foundation

[English](rfc.en.md) · [現行調査](baseline-audit.md) · [PR/検証計画](implementation-plan.md)

状態: **独立レビュー完了・採用前の提案**。2026-10-08 JST。0.2.0機能は未実装・未リリース。このRFCの保存は新しい公開仕様の採用ではない。既存のG-AUTH、Task/spawn、Tx、High/Lowの契約を維持し、[判断D1–D3](#公開判断)の採否を記録してから公開APIを実装する。

## 目的と範囲

0.1.xの型・所有権・Structured Concurrency・SQLite・HTTPの安定性を維持し、Webアプリが認証・認可・入力/出力境界を明示できる標準APIを整える。安全性は「対応する標準APIを、定義した信頼境界と配置条件で使ったときの限定された保証」であり、全アプリ、全Rust、全インターネット入力が安全になる宣言ではない。

対象はAuthScope、HTTP route方針、CSRF、HTML出力/XSS、SQL構造とbind、policy付き送信HTTP/SSRF、CORS、Cookie/Session、資源予算/DoS。OAuth/OIDC provider、password database、MFA、汎用taint/effect/region体系、PostgreSQL、独自暗号、HTTP全面置換、VM/GC/sandboxは今回追加しない。credentialsを検証する既存Rustライブラリ/adapterと、レビューするNagi policyを使う。

## 現行の基点

main `62bbda9`（#99込み）、公開0.1.11 `003a594`、open PR0。詳細とsource hashは[調査](baseline-audit.md)。現行Principal/Grant[P]はopaqueでNagiから偽造・copy/shared/JSON復元できないが、request終了・expiry・失効に連動しない。owned async delegationは許される。標準routeに認証方針引数はなく、htmlは生文字列、Cookie/Session/CSRF/CORSの標準契約と送信HTTPはない。

SQLite Parametersの値bindとnative authorizerは実装済み。SQL/schema preflightはopt-inで、動的SQLの出所やtenant policyを証明しない。HTTPにはbody/header/同時処理/各期限があり、frontendには入力/深さ制限がある。これを再実装しない。

## 信頼境界と脅威モデル

守る資産はcredential/session秘密、認証主体とresource/permissionの結び付け、非公開データ、DB整合性、サービス資源、配布物の同一性。非信頼入力はHTTP method/path/header/body、browser origin、session ID/CSRF token、SQLの値、許可先を選ぶ入力、DNS応答、外部HTTP応答。アプリsource・manifest・Rust issuer/driver・配置policyはtrustedで、内容は別途レビューする。

| 境界 | 脅威と防御の場所 | 保証の外 |
|---|---|---|
| credential → identity | duplicate/ambiguous credentialを拒否、trusted verifierがcrypto・claimsを検証 | verifier/policyを意図的に偽るRustや依存、侵害されたIdP |
| identity → operation | nominal permissionと対象実値、現在有効性の検査、DB側predicate | 全policyの正しさ・全SQLのtenant制約・検査後の外部状態変化 |
| browser → route | route方針、Session/CSRF/Origin、CORSの別責務 | GETで副作用を作るアプリ、信頼したoriginの侵害、browser extension |
| data → response | 文脈別renderer、URL検証、CSP/nosniff等の防御補助 | 任意raw HTML/JS/CSS、DTOの機密分類、全DOM情報流 |
| values → SQL | 固定構造とbind、native parse/authorizer | arbitrary SQL/Rust/既存Db exec、DB運用権限 |
| URL/DNS → socket | policyを実接続・redirect・proxy・pool再利用まで適用 | raw Rust networking、ネットワーク経路全体の信頼性 |
| work → resource | bounded admission/queue/cache/size/deadline、release/join oracle | non-yieldingの強制停止、OOM普遍回復、回線DDoS、OS隔離 |

Webセキュリティの型はproofの偽造/誤用を狭めるもので、攻撃者から届いたstringを型名だけで信用するものではない。Rust interopは既存の信頼境界であり、Nagi checkerの迂回を安全な動作と宣伝しない。

## 静的保証と実行時責務

以下はすべて**追加予定**。採用後、各項目のpositive/negative/native oracleがGREENになるまで現行保証へ格上げしない。

| 項目 | Nagi checkerで固定する契約 | runtime/配置で確認する事項 | 証明しないこと |
|---|---|---|---|
| route | canonical operationに必須policy、protected handlerの型、policy欠落/不一致を拒否 | dynamic route登録・全method/fallbackにpolicyを保持 | すべてのrouteがprivateであること、任意Rust Routerの網羅 |
| AuthScope/ScopedGrant | opaque、非Copy/Clone/Serde/shared/owned field、same-task、permission型、consume | request lease、scope ID・対象・expiry・失効、issuer/policy成功 | 無期限の有効性、policy論理、任意副作用のrollback |
| CSRF | policy source分類、sessionを使うunsafe methodのCSRF policy省略を拒否 | token/session binding、exact origin、重複/不正/欠落、一定時間内検査 | tokenだけでXSSを防ぐこと、アプリの副作用推論 |
| HTML | 対応rendererの型、slot文脈、raw stringとfragmentを区別 | text/attribute encoding、URL scheme、出力上限 | raw HTML/JS/CSS、全browserの完全な安全性 |
| SQL | security query constructorのliteral構造、Parameters、generic行型 | prepare/bind/shape、schema/NULL/type、authorizer | arbitrary dynamic SQL、権限やtenant条件の自動推論 |
| SSRF | Policy/Targetを必要とするclient操作、bare URLから暗黙変換なし | DNS/全address/実socket/redirect/proxy/TLS、deadline/response size | static URL parseだけで接続先を保証すること |
| CORS | typed config、明白なwildcard＋credentialsを拒否可能 | origin/method/header、preflight、Vary、全応答のreserved header | CSRFや認証・server間アクセスの拒否 |
| Cookie/Session | session proofをDTO/stringと区別、秘密Debug/Serdeの拒否 | parse/属性/entropy/rotation/失効/expiry/atomic store | browserが必ず属性を実施すること、全storeの可用性 |
| DoS | 型・定数の不正、有限resource構成の必須指定 | admission/counters/期限/実payload size、取消後解放 | コンパイラだけの総CPU/heap上限、全OOM・DDoS回復 |

literal以外のpolicy値を静的に解析できない場合は、runtimeのfallible constructorで拒否する。checkerをRust trait/crypto/URL/SQL parserの再実装にしない。

## High / Low / Rustの責務

新キーワード・attribute・一般effect systemを初手で導入しない。既存のimport、canonical standard resource/operation、関数、enum、Result、view、owned moveで表現できる範囲を優先する。以下のAPI名は候補で、動くコード例ではない。

1. High parser/resolverは既存構文とmodule IDを使う。checkerがresource metadata、operation Passing/borrow_owner、payload role、scope/lifecycle factsを検証する。
2. High→Lowでは型とcanonical identityを保持し、securityの受理証明やtrustedフラグをコメントから復元しない。保存Low・手書きLowも同じcheckerへ入り、自分の位置で診断する。
3. native Low/`@replace`統合後に最終check・sealed planを作る。policy欠落、偽造standard ID、消えたscope、異なるhandler型やchecked factsの欠落をdefaultで埋めない。外部からsecurity factsを与えるpublic setterは作らない。
4. emitterは確定したroute/auth/borrow/call planをRustへ実現する。clone、static化、auth推測を追加しない。Rustはtrait/Send/Sync/borrow/native実装を検査する。
5. runtimeは有限のinput validation、現在の権限/lease、native接続/SQL、終了確認を行う。compilerの成功はverifier成功・browser policy・DB結果の証明ではない。

同名のユーザー`AuthScope`/`route`/`html`/`query`をbuiltinにしない。resource genericには実payload/Arc state/indirect protocol/callback/phantomの区分を登録する。任意Rustのpayloadやsecurity意味論は推論しない。High受理後のNagi由来type/move/lifetime rejectionはP1の既存方針を維持する。

## AuthScopeと認可の案

AuthScopeをTaskの`scope`構文、SQLite Tx、DB検索条件の「scope」と混同しない。将来方針の「許可対象の実値を保持する」は維持する。request lifecycleという新しい部分は別の実値/leaseで表す。

推奨は既存Principal/Grant[P]を維持し、新しいrequest用AuthScopeとScopedGrant[P]を追加する案。初版の対象IDは現行Grantと同じi64で実値を保持し、permission Pはnominal phantom。`Grant[P, S]`へ既存arityを変更する案、任意target型Sを今回公開する案は互換性・payload検査範囲が増えるため後続比較とする。

- AuthScopeはverified subject、credential source、request固有ID、private leaseの実値を持つopaque resource。handlerへruntimeが作り、Nagiのbool/string/JSONからmintできない。認証に成功した事実と認可成功を分ける。
- ScopedGrant[P]はsubject、resource実値、request leaseを結び付ける。trusted issuerがレビュー対象policyの成功後だけ発行する。operationは別bare resource IDを取らず、Grantの実値でbindする。同一request leaseとの対応はruntime ID照合で検査し、nominal型だけで同requestと断定しない。
- AuthScope/ScopedGrantは同taskの引数/return・owned Option/Result localへ移動できる。class/enum field、shared、Actor state/message、Task capture/result、background escapeは拒否する。既存Txのsame-task検査を参考に実payload/Future引数を調べるが、Tx専用条件へ無理にauthを混ぜない。
- 初版はrequest proofのTaskへのdelegateを提供しない。asyncの同task呼出しは可。旧Principal/Grantのowned delegationは現状を維持する。将来のbackground delegationは別credential/audience/期限を必要とする別設計で、今回は追加しない。
- request dispatcherがprivate lease ownerを持つ。handler正常/Err/panic/timeout/cancelと未poll Future Dropで失効させ、auth failureのmapperも同じ境界に含める。security operationは使用時とnative admission直前にlease/expiry/失効状態を検査する。await中に期限を越えた証明を、まだadmissionしていない新しい処理へ渡さない。
- admissionは、保護operationの対象・内容に結び付いた一回限りのprivate execution permitを発行する線形化点と定義する。queue slot予約やFuture生成はadmissionではない。queue等の待機→有限resource予約→lease失効と共有する短いprivate gate内で現在時刻/有効状態を検査しpermitを発行→permitをconsumeしてnative commandへ渡す。gate内でawaitしない。dispatcher失効とpermit発行は同じgateで順序を確定する。予約待ち/検査後・permit発行前の失効は拒否し予約を解放する。期限の判定時刻はこのgate内の検査時刻で固定する。gate後に失効しても既発行permitのoperationは受理済みであり、任意副作用の停止/rollbackを約束しない。native adapterはこの保護operationのpermitのない新commandを開始せず、送信失敗・結果不明はdriverの観測に従う。単にboolをreadして後でenqueueする実装ではこの契約を満たさない。
- admission前の無効化は副作用を開始しない。admission後のDB/外部I/Oは失効・取消で巻き戻ったとは説明しない。policyとSQLのTOCTOUには同じ対象のpredicateまたはTx内の再確認が必要で、自動的に保証しない。
- 無効credential/denied/expiredは通常のSecurityFailureで、TaskFailureやsticky scope faultにしない。verifier内部panic/unexpected cancellation等は既存runtime故障の扱いを保つ。401/403/400と503/500/504を区別し、秘密/内部causeをresponseへ出さない。

JWS verifierを追加する場合は既存crateへ署名/algorithm/key処理を委譲する。algorithm allowlist、issuer/audience、exp/nbfと有限clock skew、key selection/cache上限を明示する。token由来URLへkeyを取りに行かず、startupで信頼したissuer/key sourceを指定する。固定credential sampleを本番verifierの完成証拠にしない。

## HTTPルート方針

検討する選択肢は、旧routeのarityを変更して全APIにpolicyを必須化する案A、新しいpolicy必須のapp/routeを追加して旧APIをdeprecatedにする案B、decoratorだけの暗黙保護を追加する案C。**推奨はB**。破壊を避けつつ、新しい0.2.0 Security Foundation対象appはpolicy省略で構築できない。旧APIを使うappまで自動保護したとは宣言しない。

型で区別したpolicy付きappに、public/authenticated/authorizedのいずれかを各method/path登録時に必須指定する。publicは意図的匿名公開であり、CSRF/CORS/limitsを無効化する値ではない。authenticatedはverified AuthScopeを必要とし、authorizedはさらにnamed policy/permission/対象解決を要求する。保護operation側のGrant要求も保つ。

登録policyはroute recordの実値で、dynamic pathでも保持する。policyなしの旧Appをsecurity serveへ暗黙変換しない。すべてのhandler型・mapper型とpolicy relationをsealed planで検査する。public handlerへ認証済みproofを自動発行しない。

- GET→HEAD fallbackはGETと同じpolicy。明示HEADは自分のpolicyを要求する。duplicate/重なり/同名captureは既存route条件を保つ。
- OPTIONS preflightはCORSが限られたmetadata応答として扱い、protected handlerや認証済みproofを発行しない。通常OPTIONSは明示policyで登録する。
- 404/405はtransport応答でbody/proofを公開しない。Allowのroute存在情報は秘匿保証の外と明記する。private route存在を隠すAPIを初版へ暗黙追加しない。
- legacy decorator、`serve(Db, port)`由来の組込`/health`/`/stream`/`/ws`、Rust/Axum Routerはpolicy付きappと別経路。新appへ勝手に注入しない。旧appの移行時に組込endpointを明示登録するか削除する。WebSocket upgradeの認証/session再検査は今回の新HTTP appの対象外として機能差を明記する。

pre-dispatchはconnection/header制限→path/method選択→request/security admission→曖昧header/外部origin/credential検査→認証→CSRF必要性判定→有限body/token検査→handler/認可operationの順を基準とする。単一handler期限とは別にsecurity検査の共通絶対予算を持ち、verifier/key/session待ちごとに期限をリセットしない。認証前に必要なpublic/loginのbodyも同じ有限admission/body制限を受ける。既存transport status優先順位を変更する場合は明示し、すべての早期returnでpermit/body/leaseを片付ける。

## CSRF、Cookie、Session

認証とは本人の確認、認可とは特定操作の許可、CSRF対策とはbrowserが自動添付するcredentialを第三者siteから使われる経路の制御。この三つを別policyにする。

Sessionはbrowser用host-only cookieのopaque IDを基点にserver側のbounded storeで主体/expiry/失効世代を保持する案を推奨する。Cookieは`Secure`/`HttpOnly`/`Path=/`と明示SameSite、Domain省略の`__Host-`形状を検証する。SameSite=NoneはSecureを必要とし、cross-origin用途を明示する。正確なparser/serializerは維持されているcrateを比較し、独自cookie grammarを作らない。複数Cookie headerを含めて解析し、認証用の同名cookieが重複する場合は選び直さず拒否する。

標準Session発行/rotation/認証状態を含む応答とCSRF token配布応答は、新appのfinalizerが`Cache-Control: no-store`を所有する。Set-Cookieだけでcache禁止と扱わない。handlerがpublic/max-age等の競合cache headerを加えた場合は拒否し、304/共有cacheによるtoken/session応答再利用をしない。発行/token応答には明示したcookie/credential source/認証依存のVaryを併用するが、Varyをno-storeの代わりにしない。普通のDTOの機密性・cache policyを自動推論したとは説明せず、app作者が別途指定する。

session IDはOS CSPRNGから少なくとも128bitの推測不能性を持つopaque値。secretのDebug/Serde/response echoを提供しない。login/権限変更時にrotationし、atomicに旧ID無効化と新ID発行を行う。idleとabsolute expiry、logout/失効、同時request/rotation、clock rollbackの挙動を定義する。cookie削除はserver失効の代用ではない。storeの停止/応答喪失を認証成功へ変換しない。

初版のreference storeは明示capacity/expiryの単一processメモリstore候補。再起動でlogout、多instanceの共有保証なしを契約にする。production store adapterはlookup/rotate/revokeのatomic契約を必要とし、単純なMap put/getの成功を線形性の証明にしない。SQLite store案と比較し、永続/多instance要件をD3で固定する。

GET/HEAD/OPTIONSをsafe methodとして扱うが、そのhandlerに副作用がないことをcheckerは証明しない。cookie等のambient credentialを使うunsafe methodは、**public/login/logoutを含め**CSRF policy必須。初版はsessionまたはpre-loginの有限anonymous sessionにboundしたsynchronizer tokenを推奨する。token長/読取上限、CSPRNG、constant-time比較、rotationを既存crypto部品へ委譲する。tokenをURL/query/logに入れない。form bodyはboundedかつ一意tokenを要求し、JSON APIでは専用headerを使う。

Originは設定済み外部origin（scheme/host/port）とexactに比較し、opaque/null/重複/不正/欠落はbrowser-sessionのunsafe requestでは拒否する案。Referer fallbackや任意Origin反射を初版に入れない。SameSiteとFetch Metadataは追加防御で、tokenや認可の代わりではない。browser互換性は統合試験で確認する。

header-only service credentialを使いCookieを認証に一切採用しないrouteはCSRFをappが免除するbooleanではなく、そのcredential source型により区別する。複数sourceの混在/曖昧なfallbackは拒否する。署名webhook等の別credential方式は専用verifier契約へ分け、汎用`disable_security`操作を追加しない。

TLSは既存どおりfront proxyで終端できる。外部originはstartup configを正とし、Host/X-Forwarded-*/Forwardedを無条件に信じない。proxy peerを信頼する場合はlistener/Rust hostの明示した信頼設定が必要。公開Session sampleは実TLS/front proxyでcookie送受信を確認し、Secureを無効化する開発用bypassを標準APIへ足さない。

## CORS

CORSはbrowserによるcross-origin response読取規則で、認証/認可/CSRFを提供しない。既定はcross-origin読取不許可。明示allowlistのscheme/host/port、method/header、credentials、有限preflight cacheをtyped policyで持つ。正規URL/origin parserを再利用し、suffix/substring/正規表現の曖昧一致や入力Originの無条件反射はしない。

credentialsとwildcard originの併用、null origin、複数Originは拒否する。許可originのみ反映し、`Vary: Origin`とpreflightのmethod/header依存を付ける。401/403/413/429/500等の早期応答にもpolicyを適用するが、許可されないoriginへcredential responseを読ませない。preflight成功はactual request成功を保証せず、actual requestで再度auth/CSRFを検査する。

新appではsecurity-managedのCORS/session/CSP等headerを統合finalizerが所有し、handlerのappend headerで二重・矛盾値を上書きできない。旧raw response header APIの意味は維持する。reserved集合とreject段階をcontract testで固定する。

## XSSとHTML出力

JSON/plain text出力は現行APIを使い、必要なContent-Typeと`nosniff`を新appのfinalizerで揃える。JSON encodingがscript埋込やHTML attribute向けencodingとは説明しない。

初版は既存string interpolationで安全を推測せず、opaque HtmlFragment等のtyped builderで対応するHTML subsetを構築する案。text node、quoted attribute、許可URL attributeを別operationにし、component/fragmentの結合もcontextを保つ。要素/attribute名は静的allowlistで、event handler、script/style、rawtext、任意CSS、srcdoc、非対応namespaceを拒否する。encodingが必要な文字・Unicode・NUL・再encodingの挙動と出力budgetを固定する。

URL attributeはHTML encodingとscheme/用途の検証を両方必要とする。ナビゲーションURLとSSRF用network Targetは別型にし、hrefが有効だからserverが取得してよいとはしない。任意HTMLを「洗う」sanitizer、文字列をそのままtrusted fragmentへcastするNagi factoryは初版に提供しない。

旧`Html`/`html`/`http.html`はraw互換APIで、XSS保証外を日英Docsに明示する。新appはtyped rendererを推奨し、raw APIへのwarning/後の制限をD1のmigrationに含める。CSPのrestrictive profileは防御補助で、encoding/認可の代用ではない。nonceが必要な拡張はrequest生成・秘密管理・template統合を別に設計し、unsafe inlineを黙って許可しない。

## SQL Injection

既存SQLite Pool/Tx/Parameters、authorizer、結果shape/cleanup契約を維持する。security queryの新constructorは**直接literalのSQL構造**をopaque Queryへ封印し、値はParametersで渡す案。変数/連結/formatで作ったSQLをこのconstructorへ渡すとchecker拒否。canonical operation identityで判定し、任意ユーザー関数のstringを拒否しない。

literalのSQL構文をcompilerに独自実装せず、実行時SQLite prepareの一文/bind/shape制約を保持する。literalでも権限のあるSQLを必ず良い操作とは呼ばない。identifier/order等の選択はレビュー済みliteral queryから選ぶ。任意identifier quotingやSQL escapingでvalue bindを代替しない。

既存dynamic query/旧Db.execは互換を維持し、trusted structureをappが保証する経路として明示する。新Queryを既存Sqlと暗黙変換せず、新operationがopaque Queryを必須にする。通常checkへschema engine必須依存を追加しない。値型/NULL/bind長/schema差はnative実行で確認し、G-SQLのopt-inと別の保証IDで記録する。

ScopedGrantでprotected DB operationへ入る場合はGrant内対象実値をbindし、owner/tenant等をSQL predicateか同Txで再確認する。汎用queryにGrantを渡せば全SQLのtenant制約が成立するAPIは作らない。取消/lease失効/Errを自動rollback成功へ変換しない。

## SSRFと送信HTTP

標準client未実装の現状から、既存Rust client/URL/TLSを比較する独立PRで始める。URL文字列validatorだけを「SSRF対策済み」と出荷しない。初版はstartupで設定した有限のサービスorigin allowlistと目的別DestinationPolicyを必要とする。HTTP clientの任意URL APIを先に公開しない。

- URLを一度canonical parseし、scheme/host/port/userinfoを検査。既定HTTPS、userinfo/fragment拒否。相対path/queryの値は構造と分け、base originを変えない。HTTPS証明書/hostname検証を保持する。
- 名前のallowlistとaddress制約は別々に満たす。DNSの全候補addressにpolicyを適用し、承認したaddressに接続を結び付ける。再解決・IPv4/IPv6・mapped address・retry/Happy Eyeballs・connection pool再利用にも同じpolicyを適用する。
- redirectは初版で追従しない。将来追従を追加するなら各hopを再検証し、別originへcredentialsをforwardしない。環境proxyを暗黙採用しない。proxyがDNS/接続を所有する場合は同等policyを実証できるtrusted adapterとして別に扱う。
- private/link-local/loopback/metadata等は既定拒否。内部サービス用途はstartupの固定service identityと限定address rangeを別policyとして明示し、request由来入力で許可範囲を広げない。`allow_all`/TLS verify無効/URL受理だけのbypassは提供しない。
- connect/resolve/whole-requestの絶対予算、response bytes/redirect数/接続容量/queue/cache上限を明示し、timeout・policy denial・HTTP業務status・transport failureを分ける。decoded bodyも上限対象。response終了/取消時のconnection再利用と解放を検査する。

実socketと検証済みaddressの対応を選んだcrateで維持できなければSSRF PRはblocker。DNS mockの成功だけで完成にしない。internet general browsing clientは初版の対象外。

## DoSとlifecycle

既存HTTP/body/header/capacity/send/shutdown、Actor/Task、SQLiteの取得budget/queue/native close/join、frontend入力/深さ上限を保つ。SQL取得期限をSQL実行/BEGIN/busy期限に広げない。

新機能はcredential/cookie/token/URL/input長、verifier同時数/key cache、Session総entry/expiry cleanup、CSRF状態、CORS allowlist/cache、renderer/response出力、DNS/address/cache/client queue等の有限budgetを必要とする。request由来keyで無限にMapが伸びる構造を避ける。共有部のoverflow/最大native容量/有限期限をfallibleに検査する。

rate limitingはauth主体/route等のbounded identityと明示policyで提供する案。client-supplied IP/headerを本人と扱わない。peer情報が必要ならtransport/信頼proxy境界を先に完成する。rate limitはbyte/capacity/期限の代用でも分散quotaの保証でもない。0msの意味はAPIごとに固定し、HTTP現行のpositive deadlineとSQLite即時取得を機械的に統一しない。

すべてのnormal/denied/Err/panic/unpolled/Pending cancel/shutdown経路でpermit・lease・workerを片付け、actual joinが必要な場所は通知と区別する。opaque secretをDebug/log/responseに出さず、内部causeをoperator diagnosticと公開error codeに分ける。blocking crypto/DNS/native workを同期Dropで終了確認したと説明しない。

## 互換性と移行

既存move、borrow origin、正常Task出口義務、業務Err/sticky fault、legacy spawn、Supervisor/HTTP、Tx affine/close/outcomeを維持する。High/Lowの同時移行を必須とし、保存Low metadataの既存拒否条件を弱めない。Rust public constructorの削除やGrant arity変更は行わない。

推奨Bでは0.2.0でpolicy付きHTTP app/route、AuthScope/ScopedGrant、HtmlFragment、Query、Target/Cookie/Session等を追加する。旧app/decorator/raw HTML/dynamic SQLはdeprecated/保証外境界として明示し、専用migration guideと移行後native sampleを提供する。警告はcompiler check・editor・saved/hand Lowで同じ意味にする。警告を抑えるsecurity bypassやファイル拡張子による免除を作らない。

全標準HTTPにpolicyを必須化して旧route署名/handlerを削除する案Aは公開breaking changeであり、別途承認が必要。単に0.2という版番号だから自由に破壊できるとは判断しない。新appからlegacy serve/routeへ暗黙downgradeする操作は設けない。安全性を主張する範囲をcompiler diagnostic・Docs・配布API差分で同じにする。

## 公開判断

| ID | 比較と推奨 | 利用者への影響・採用前のblocker |
|---|---|---|
| D1 | A=旧HTTPもpolicy必須へ破壊、B=新policy必須Appを追加＋旧経路deprecated、C=decoratorだけ保護。**B推奨** | 全Nagi HTTP routeの一律義務ではなく新appの網羅保証。既存署名維持、移行は明示。全経路一律義務が必要ならAと組込endpoint/Low/Rust adapterの移行を選ぶ必要がある |
| D2 | 旧Grant arity変更、既存Grantに失効を付加、新AuthScope/ScopedGrant併存。**追加型とsame-task request proof推奨** | 旧delegation保持、新request proofはTask/Actor転送不可。将来の任意target型やbackground delegationは別設計。公開名/対象型/constructor/handler signatureをADRで固定する |
| D3 | server store型Session（bounded memory/SQLite/trusted external）とstateless signed cookieを比較。**server store＋host-only cookie、初版bounded memoryとatomic adapter契約推奨** | 再起動でlogout、多instanceには対応store必須。stateless cookieは即時失効/rotationが複雑。production persistenceが0.2必須ならSQLite adapterをacceptanceへ加える。新crypto/cookie/client crateは実source/保守/license/4 OSで選定し、採用差分を記録する |

これらは提案であり、旧ADRの未承認部分を採用済みと書き換えない。次段階は判断をADRへ固定→専用RED→実装。機能別PR、独立レビューと各検証のacceptance、全体release gateは[実装計画](implementation-plan.md)にある。

## 指針と保証の限界

参照: OWASP [Authentication](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)、[Authorization](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html)、[CSRF](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html)、[XSS](https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html)、[SQLi](https://cheatsheetseries.owasp.org/cheatsheets/SQL_Injection_Prevention_Cheat_Sheet.html)、[SSRF](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html)、[Session](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)、[DoS](https://cheatsheetseries.owasp.org/cheatsheets/Denial_of_Service_Cheat_Sheet.html)、[WHATWG Fetch/CORS](https://fetch.spec.whatwg.org/#http-cors-protocol)、[RFC6265](https://www.rfc-editor.org/rfc/rfc6265)、[RFC9110](https://www.rfc-editor.org/rfc/rfc9110)、[RFC8725 JWT BCP](https://www.rfc-editor.org/rfc/rfc8725)。SameSite/prefixはRFC6265だけの保証ではなく現代browserの検証が必要。[取得記録](reference-retrieval.json)。

このRFCはsecurity certificationではない。runtime未実装、依存比較・browser/proxy/実DNS/TLS・migration手順の実行は今後のacceptance。無関係な最高難度modelの重複調査、汎用攻撃自動化、第三者への検証、無制限資源実験は実装計画へ含めない。
