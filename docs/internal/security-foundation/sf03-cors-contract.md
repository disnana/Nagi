# SF03 CORS 契約差分 v1（未検証実装候補）

採用RFCの具体化。既存18件のnull期待をここで仕様へ解決する。runtime/compiler source と31件のoracle計画があるが、parse/check/native/socket/browser/4OSは全てUNRUNである。公開APIはこの未検証候補内の登録であり、安定版で利用可能という意味ではない。B1 Session CSRF Draftへの依存、実compilerによるgolden再生成、独立監査、全OS CIが公開受理の前提である。

## 最小の公開API

`http` は `std.http.server`。新keyword、annotation、全route arity変更は不要。

```text
http.CorsCredentials = OMIT | ALLOW
http.CorsPolicy

http.cors_policy(
    origins: List[str], request_headers: List[str],
    credentials: CorsCredentials, max_age_seconds: i64,
    max_origins: i64, max_origin_bytes: i64,
    max_headers: i64, max_header_bytes: i64
) -> Result[CorsPolicy, http.Error]

http.cors_route[S,E](
    app: App[S,E], method: Method, registered_path: view[str], cors: CorsPolicy
) -> Result[App[S,E], http.Error]
```

`CorsPolicy` はopaque、非秘密・非Copy・非Serde・equalityなし、storage/shared/Debug可、暗黙Cloneなし。generic roleなし。immutable configとしてruntimeのroute recordが所有する。`CorsCredentials` はCopy/equality/storage/shared/Debug可のcanonical enum。ALLOWはcredentialed browser readを許す値で、credentialの存在や認証成功を作らない。OMITはAllow-Credentials headerを出さない値で、serverへ届くcredentialを取り除いたり既存認証を免除したりしない。

registry Passing は `cors_policy` 全8引数Move、`cors_route` が `[Move, Move, Reference, Move]`、borrow_ownerなし、両sync。戻る Result/Errorは既存 HTTP型。`cors_route`のgeneric S/Eは既存Appと同じpayload/protocol区分で、CORSにproof特別ルールを加えない。想定Rust exportは `::nagi_runtime::http_server::{CorsPolicy,CorsCredentials,cors_policy,cors_route}`。新moduleの内部実装をpublic別dispatcherとして再exportしない。

`registered_path` は `app.routes` に保存された**登録patternそのもの**。具体request path、query、URI正規化、matchitのbest-match検索を設定書込に使わない。動的patternも同じfallible API。対応record無し、同recordへ二度付与、CORS用method `*` / CONNECT / TRACE / TRACK、methodのHeaderValue化失敗またはtoken bytesがmax_header_bytes超過は Error::Invalid。後3つの通常route登録/既存CONNECT501契約は変更しない。GET fallbackだけがGETのconfigを継承する。明示HEADはGETとは独立。token/Bootstrapが生成する明示GET/HEAD recordへは各々設定が必要。

## Originと有限設定

grammar は小文字 `http://` または `https://` + 現行のchecked Authorityだけ。scheme・authority以外のcomponentはない。hostは現行ASCII DNS（casefold、長さ/label条件、末尾dotなし）かcanonical IPv4/IPv6。IPv6はbracket必須。IDNAはdeployment側でASCII A-labelへ。portは1..65535のleading-zeroなしdecimal、省略はhttp80/https443へ。tuple equalityにより明示default portと省略は同値。scheme、非default port、hostの違いは異なるorigin。IPv6の同じ値/表記差は正規化。userinfo/percent/whitespace/区切りcomma/Unicode/null/opaque/path（末尾 `/` も）/query/fragment/wildcardは不可。

parserは現行 `http::Uri/Authority` + checked Host条件を一つのprivate helperへ集約し、独自の第二URL grammarや新依存を追加しない。**ExternalOriginのHTTPS制約・wire Hostのport省略と明示の別allowlist・trusted peer判定は変更しない。** CORSのHTTP source originを明示許可できても、CSRF origin比較はHTTPS ExternalOriginのまま。

`max_origins/max_origin_bytes/max_headers/max_header_bytes` は positive_size と同じnative範囲へfallible変換。allowlist/header listは空でも可。各raw listのcountと総bytesを正規化/新allocation前にchecked計算し、canonical出力のbytesも同じ上限内。header総bytesは名前とserializationの `, ` 区切りbytesを含む。originのcanonical重複、headerのcasefold重複はError::Invalid。request header名は現行 `HeaderName` parserで検査・lowercase化し、`*`を許さない。方法としてCORS wildcardは全credentials modeで提供しない。count+byteの限界±1/整数overflowをstartup constructorに固定する。max-ageは0..u32::MAX秒、0でも `Access-Control-Max-Age: 0` としpositive lifetimeを作らない。配備者の明示値でありproduction defaultはない。

許可Origin値はconfig内のcanonical HeaderValueを使い、raw request文字列を返さない。request由来Origin/header組合せをpersistent Map/cacheへ追加しない。server preflight cacheは0entry。設定メモリは各有限recordのchecked count/bytesの和で、trusted App登録数・trusted startup loop自体は既存境界。HTTP head全体は既存bytes/count/deadlineの上限を受け、選択済みCORS recordではOrigin長とrequested header count/bytesをconfig上限内で検査する。

## Dispatch・status・wire

1. 既存connection/header framing・Host/peer/URI/Forwarded検査を先に行う。拒否は既存status/no-store/close、CORS headerなし。pathだけによる既存CSRF管理metadata付与は弱めない。
2. OPTIONSで ACRM または ACRH があればpreflight候補。それ以外はordinary OPTIONSで、明示OPTIONS Policyが必要。OriginだけのOPTIONSはordinary。非OPTIONSのACRM/ACRHはCORS dispatcher選択に用いず、普通のrequestとして既存処理を保持する。
3. preflight候補のACRMは一つの非空Method tokenであることが必要。duplicate/欠落/不正/`*`は400。Methodはcase-sensitiveで、既存parserのcustom methodもexactに扱う（CORS非対応methodは400）。この新metadata経路の診断はpolicy/handlerを実行しない。
4. ACRMをactual methodとして既存path/method選択（明示HEAD優先、HEAD→GET fallback）。unknown path404、methodなし405/元Allow。両者にCORS grantなし、Varyはpreflight3依存、no-store。preflightは明示OPTIONS routeを横取りして呼ばない。requested method自身がOPTIONSならそのrecordのmetadataだけ使う。
5. 選択済みrouteでは既存request permitを取得（不足503）、宣言Content-Length上限を確認（超過413）。これらstatusは後のOrigin/CSRFエラーで上書きしない。preflightはbodyを読み込まず、nonempty Incoming/Transfer-Encoding/positive Content-Lengthは400+close。0/不在だけが可。全returnでpermit/bodyを解放する。
6. preflightのOriginは単一必須。欠落/duplicate/invalidは400、null/valid-unlisted/default-denyは403。単一ACRHは不在なら空集合、ある場合はHTTP tokenをcomma-listで分割しOWSを許してlowercase化。空segment/empty field/duplicate field/重複name/不正/`*`/config count・byte超過は400、valid-unlisted headerは403。header集合はconfigの部分集合であることが必要。
7. 全条件成功は204。ACAOはcanonical exact origin、ACACはALLOWなら一つの `true`、OMITなら不在。ACAMは選ばれた要求method一つ、ACAHは要求された許可headerだけをlowercase・lexical sortで `, ` 接続（空なら不在）、Max-Ageは明示seconds。body/framingは既存204規則、nosniff、Cache-Controlはno-store。preflightエラーではACAM/ACAH/Max-Ageを出さず、routeが選べてvalid許可OriginならACAO/ACACだけを適用できる。非2xxでpreflightは許可されない。

preflightはmetadataのみなのでtarget routeの認証/CSRF token/sourceを評価しない。Cookie/Authorizationが送られていてもそれを採用せず、proof/owner/DB/cookie配送0。VaryはOrigin、ACRM、ACRHのみで、target sourceの不要なcredential依存を新たに作らない。成功もHTTP no-storeだが、browserのCORS-preflight特別cacheはAC-Max-Ageで別管理される。browserの実効capやentry数をNagiが保証したとは説明しない。

actualはrouteのCORS configがあり、単一valid Originがallowlistに一致した時だけACAO/ACACを付ける。unlisted/null/duplicate/malformed/missing OriginはCORS grantなしだが、CORSだけでhandler0や400/403を新たに要求しない。全actual認証/CSRFは現行順に実行し、既存原因のstatusを維持する。選択済みrouteの401/403/413/500/503/504等、mapper/handler Err/panic/timeout、専用endpointのfixed success/失敗にも同じcontextを適用する。429はproducerが存在する場合だけ。Originが不許可ならcredential responseをbrowserに読ませない。

unsafe SessionのCSRFは設定済みexternal HTTPS originとtoken/session bindingが引き続き必須。別のallowlisted originからのpreflight204後にguarded POST403が返るのは正しい動作で、valid tokenでもhandler0。CORSにALLOWを設定してもunsafe Session、BrowserLogin、Bootstrap/token endpointの独立Origin checkを緩めない。safe Session GETでcross-origin読取を検証し、cross-origin unsafe success fixtureを作らない。

## Vary・no-store・秘密配送の合成

外側dispatchは非秘密 `CorsContext`（selected record、Allowed config index/Denied/Missing、Actual/Preflight）をoriginal headからboundedに作り、await・Cookie/token stripで失わない。private `ManagedDependencies` をresult responseと別に保持する。guarded/token/Bootstrap/Browser return、transport/mapper/304→500等のreplacementを通ってもrequest base依存を残す。delivery結果の追加Cookie依存をこのmetadataへ返し、raw responseやheaderからproofを復元しない。

| actual source / kind | 常にunionするsecurity依存 |
|---|---|
| Public Ordinary | なし（CORS設定recordならOriginを追加） |
| Bearer Ordinary | Authorization, Cookie（Cookie混在拒否を含む） |
| Session Ordinary | Cookie, Authorization（Authorization混在拒否を含む） |
| Session Guarded | Cookie, Authorization, Origin, X-CSRF-Token |
| CsrfTokens / Bootstrap | Cookie, Authorization, Origin |
| BrowserLogin | Cookie, Authorization, Origin, X-CSRF-Token, Content-Type（JSON source検査） |
| 有効なSession配送seal / cookie delivery | Cookieを追加、元source依存を残す |
| preflight success / denial / route-less error | Origin, Access-Control-Request-Method, Access-Control-Request-Headers（credential検査/配送はしない） |

「常に」は同じroute sourceの早期拒否も含む。public handlerの任意header依存やDTO機密分類を推論する保証ではない。body/framing/admissionの既存transport条件は既存no-store/closeで扱い、ここはframeworkのCORS/credential/CSRF/login sourceの依存を合成する。source側のfixed insert・「Varyがあれば追加しない」分岐・Bearer cookie成功時のinsertを廃止し、boundedなprivate enum bitsetのunionを一つのwriterで一回serializeする。順序は `Access-Control-Request-Headers, Access-Control-Request-Method, Authorization, Content-Type, Cookie, Origin, X-CSRF-Token`。case-insensitive重複0、header field一つ、`*`なし。CORSはconfigured actual recordの許可/不許可/Origin欠落を含めOrigin依存を追加する。grantのないunknown404/405やauthority拒否に他routeのCORSを推測しない。

このwriterはSession secret take/retireとHTML/CSP/body変換の**後**、send開始前に実行する。sourceによるno-store、seal不正/cache競合/304の秘密なし500、Set-Cookie無し、mapper replacementのdelivery禁止を保持する。CORSはCache-Controlをcacheableへ変更しない。普通のpublic DTOの304は既存framingを維持してCORS/Varyを付け、authenticated/Session/tokenの304は既存500+no-storeへ置換した後CORS/Varyを付ける。Set-Cookie複数append、CSP/nosniff、HEAD/body/framing、WWW-Authenticateを上書きしない。

managed headerのuser append拒否は既存runtime Error::Invalidのまま。string内容の拒否をchecker成功negativeに読み替えない。cancel/connection closeには架空wire responseを要求せず、permit/lease/delivery解放と次の所有requestの完了を観測する。non-yielding処理の強制停止や受理済み外部効果rollbackは追加しない。

Nagiの既存リスト型は `List[str]`、Rust adapterでの引数は `Vec<String>` である。既存Appのroleは SharedPayload(S) と CallbackSignature(E) を保持する。手書きLowは独立入力で、保存LowのHigh原本を削除するnative fixtureと区別する。source fixtureの存在を成功receiptと数えない。TLS上のbrowserによるcredentialed read、preflight特別cache、各engineの動作は追加受理gateである。
