# SF04 typed HTML: 公開契約（freeze済み・開発中）

[English contract](sf04-contract.en.md) · [実装/RED対応表](sf04-implementation-map.md) · [採用RFC](rfc.md) · [D1–D3](decisions-and-migration.md)

2026-10-10 UTC。**SF04-C01–C08は採用RFC内の具体実装契約としてfreeze済み。API/数値は下記のまま。feature全体は未実装/未acceptance。** 固定baseは actual main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`（tree `9172331188d0d39f6e91ca29c47e6ec9cf691ce5`）。比較用SF05は `47dfc09ab36e553e45f8b49b1b3eba5ee16ddc7b`。古いlocal main、SF05/reader/editor WIPを含めない。SF04の直接依存はmerged SF01のみ（[計画](implementation-plan.md)のSF04行）。この文書は後続機能のacceptanceや先行CI成功を代替しない。

## 採用済み境界と残判断

RFCの「XSSとHTML出力」127–135行とD1は、opaque typed HTML、別々のtext/quoted attribute/allowed URL操作、static allowlist、context保持、有限budget、raw入口削除、finalizer所有のContent-Type/body kind/security headerを要求する。任意HTMLのsanitizer、string/bytes cast、JS/CSS/rawtext、event handlers、srcdoc、非対応namespace、unsafe-inlineを提供しない。navigation URLはserverのSSRF Targetと別型。CSPはencoding/認可の代用ではない。

次のIDはRFCの具体化で、独立review/R01再確認を経て6ef4d21でfreezeした実装契約。D1–D3自体の再承認は求めない。

| ID | 凍結した具体選択 | 安全影響・互換性・判断点 |
|---|---|---|
| SF04-C01 | 下記13操作、6 resource、owned transform、immutable dataのstorage/shared | taskをまたぐ安全なencoded dataを許す。proofではない。native Send/Syncと通常checkerの一致を受入時に確認 |
| SF04-C02 | inert 12 tag、title/hrefだけ、closed content model | 最小の一覧/詳細UI。既存任意HTML/forms/style/scriptとの完全互換ではない |
| SF04-C03 | 五文字escape、Unicode保持、NUL/所定controls拒否 | 二重解釈を防ぐ。NUL置換やUnicode正規化を期待した旧UIは移行が必要 |
| SF04-C04 | HTTPS固定base + same-origin root-relative hrefのみ | navigationスロットを狭める。外部リンク/absolute hrefは初版非対応。SSRF保証はしない |
| SF04-C05 | 下記hard ceilings、profile一致、明示bounded copy | per-valueの保持量/生成量を有限化。app全体のメモリ/CPU/取消時間上限ではない |
| SF04-C06 | private BodyKind、typed document消費、全標準応答CSP | 現在ない保証の追加。CSPの全応答適用とHTML Content-Encoding拒否の互換性をreview |
| SF04-C07 | 開発用url 2.5.8、固定context encoderを小さく実装 | exact graphはreview済み、保守/advisory/配布/targetの残条件は開発用採用判断へ |
| SF04-C08 | 通常Error、raw migration診断維持、業務app/DOM oracle | stub未定義やparser failureを機能REDと数えない。旧raw移行coverageは別commit |


通常のAPI名/subset/数値/resource flagsの具体化は採用RFC内の実装判断として独立reviewを経て固定済み。新keyword/effect/独立checkerやTask/move/Grant/Tx意味変更、保証の弱化、未採用unchecked互換APIを選ぶ権限はこの契約freezeに含めない。そのような別の公開意味論変更が必要なら、影響と代替を示してユーザー判断へ戻す。

## Resourceと所有権 — SF04-C01

新module契約は `std.html`（例のalias `html`）、nativeは `::nagi_runtime::html`。全resourceはgeneric arity 0、generic role集合とinline_type_argumentsは空。HtmlTag以外はprivate fieldsのopaque owned値。encoded文字列、URL、policy、summaryのpublic accessorは設けない。

| Resource | Copy | Equality | Storage | Shared | Debug | Serde | Lifecycle |
|---|---|---|---|---|---|---|---|
| HtmlPolicy | false | false | true | true | false | false | Unspecified |
| HtmlAttributes | false | false | true | true | false | false | Unspecified |
| NavigationUrl | false | false | true | true | false | false | Unspecified |
| HtmlFragment | false | false | true | true | false | false | Unspecified |
| HtmlDocument | false | false | true | true | false | false | Unspecified |
| HtmlTag | true | true | true | true | true | false | Unspecified |

Storage/Sharedは通常の既存metadataの意味で使い、新しいTask規則を追加しない。encoded dataは権限/認証proofではなく、owned Task transferと既存shared container利用を許す契約。SameTaskのAuthScope/Grant、Tx、move/取消契約は不変。HtmlPolicyの内部共有profileにsecret/Grant/Txを入れない。native immutable実装のSend/Syncと最終Rust trait確認を行い、受理したNagiの生成move/lifetime不整合をrustcへ先送りしない。

HtmlTagのみnative Copy/Cloneを持つ。それ以外にpublic Clone/Default/From[str]/From[bytes]/Deserialize/Display/raw factoryは設けず、Copy/Serde派生も拒否する。Debugはopaque5型で拒否し、Responseの既存status/header_count/body_bytesだけのDebugは維持する。複製は `copy_fragment` の有限な明示操作だけ。storage/sharedが許す実体移動はClone許可を意味しない。

| Operation signature | Passing |
|---|---|
| `html.policy(base_origin: view[str]) -> Result[HtmlPolicy, Error]` | Reference |
| `html.limits(policy: HtmlPolicy, output_bytes: i64, input_bytes: i64, nodes: i64, depth: i64, urls: i64, url_bytes: i64) -> Result[HtmlPolicy, Error]` | Move × 7 |
| `html.empty(policy: view[HtmlPolicy]) -> HtmlFragment` | Reference |
| `html.text(policy: view[HtmlPolicy], value: view[str]) -> Result[HtmlFragment, Error]` | Reference, Reference |
| `html.attributes(policy: view[HtmlPolicy]) -> HtmlAttributes` | Reference |
| `html.title(attributes: HtmlAttributes, value: view[str]) -> Result[HtmlAttributes, Error]` | Move, Reference |
| `html.href(attributes: HtmlAttributes, value: NavigationUrl) -> Result[HtmlAttributes, Error]` | Move, Move |
| `html.navigation(policy: view[HtmlPolicy], value: view[str]) -> Result[NavigationUrl, Error]` | Reference, Reference |
| `html.element(policy: view[HtmlPolicy], tag: HtmlTag, attributes: HtmlAttributes, children: HtmlFragment) -> Result[HtmlFragment, Error]` | Reference, Move, Move, Move |
| `html.join(policy: view[HtmlPolicy], left: HtmlFragment, right: HtmlFragment) -> Result[HtmlFragment, Error]` | Reference, Move, Move |
| `html.copy_fragment(policy: view[HtmlPolicy], fragment: view[HtmlFragment]) -> Result[HtmlFragment, Error]` | Reference, Reference |
| `html.document(policy: view[HtmlPolicy], title: view[str], body: HtmlFragment) -> Result[HtmlDocument, Error]` | Reference, Reference, Move |
| `http.html_response(status: http.Status, document: html.HtmlDocument) -> http.Response` | Move, Move |

全操作は同期、generic arity 0、`emit_type_args=false`、`borrow_owner=None`。所有する戻り値を作り、viewを返さない。明示 `view[...]` 入力は既存のPassing::Reference、owning/scalar引数はMove（HtmlTag自体はCopy）。callerのviewはcall終了を越えて保持しない。policy profileはruntime内部でowned immutableに保持し、Reference入力のlifetimeを延長しない。

標準Rustのpublic APIも同じ型/順序/結果で `view[T]` を `&T`、`str`を`&str`、`i64`を`i64`、Errorを既存runtime Errorへ写す。`http_server::html_response(Status, html::HtmlDocument) -> Response` が唯一のHTML response入口の契約。低levelunchecked constructor、public fields/body-kind setter、unsafe trait conversion、ambient string/bytes HTML response factoryを標準Rustにも残さない。カスタムnative Rust/unsafe externの任意active配信は別のtrusted-host著者境界であり、標準APIの迂回として案内しない。

## Subsetと構造 — SF04-C02

HtmlTag constantsは `DIV, SPAN, P, H1, H2, STRONG, EM, UL, OL, LI, A, BR`（12種類）。`title`は全user tagに任意で1個、`href`はAにだけ任意で1個。重複はError::Invalid、serialization順はtitle→href。動的tag/attribute名、id/class/data-/aria-、forms/images/media/table/iframe/template/object/embed/link/base/meta、CSS/JS/event/srcdoc、SVG/MathMLを公開しない。任意名を取って後でsanitizeする操作はない。

flowはphrasing＋DIV/P/H1/H2/UL/OLであり、LIはlistの子としてだけ許す。P/H1/H2/SPAN/STRONG/EM/Aはphrasing（text、BR、SPAN、STRONG、EM、A）だけを子に持つ。Aの子孫にAがある場合は拒否。DIV/LIはflow/phrasingを受け、UL/OLは空またはLIの列だけ（whitespace textも不可）。documentのbodyはflow/phrasingを受け、LIの直置きを拒否する。BRはempty childrenだけ、void serializationで閉じtagを出さない。閉じた構造summaryでbrowserのimplicit close/foster parentingを避ける契約であり、実browser構造oracleは未実行。

`join`はsiblingsを連結し、`element`/`document`はchildren summaryのcontent modelを検査する。孤立LIはUL/OLへ組み込む途中のfragmentとしてのみ保持できる。join自体はparentを仮定しない。深さ/数/anchor有無/top-level分類を私有summaryに持つ。HTML文字列を再parseして型を回復しない。

`document`だけが固定wrapperを作る。head/meta/titleは外部のHtmlTagに出さず、titleはplain strを専用text encodingして固定RCDATAへ置く。bodyにはtyped fragmentだけを置く。

```html
<!doctype html><html><head><meta charset="utf-8"><title>{encoded title}</title></head><body>{typed fragment}</body></html>
```

## Text/quoted attribute/Unicode — SF04-C03

text、title attribute、document titleは別操作/固定contextで出力する。五文字を常に `&`→`&amp;`、`<`→`&lt;`、`>`→`&gt;`、`"`→`&quot;`、`'`→`&#39;` とescapeする。attributeはdouble quoteで囲む。document titleも`&`/`<`をescapeしてtitle終端として再解釈されないようにする。

UTF-8 strのUnicode scalarは正規化せず保つ。U+0000はInvalid。C0とDELはTAB/LF/CR以外Invalid、許す三文字は `&#9;` / `&#10;` / `&#13;` で出し、browser newline normalizationを避ける。raw byte→str/fragment castはない。通常のUnicode、quotes、ampersandだけの小fixtureでlogical DOM text/quoted valueを検証する。

plain入力 `&amp;` はliteral textとして `&amp;amp;` になる。encoded fragment同士のjoin/copy/elementは既存encoded bytesを再escapeしない。URLは先に用途/canonicalizationを確認してからquoted-attribute encodingする。JSON encoderをHTML/JS encoderとして流用しない。

## NavigationUrl — SF04-C04/C07

`policy`はbounded raw HTTPS base originをparseし、typed scheme/host/effective portでcanonicalizeする。userinfo（空の`@`も含む）、query/fragment、非root path、root dotを拒否する。既定portはcanonicalizeする。domainはcanonical ASCIIでlabel/domain長を検査し、IPv4/IPv6 literalはparserのtyped Hostを使う。originをraw suffix/substringで比較しない。

`navigation`は単一leading `/` のroot-relative path/query/fragmentだけを受ける。`//`、backslash、全C0/DEL/ASCII spaceをraw input段階で拒否する。absolute URLやscheme付き入力は不可。固定baseへparserでjoinし、最終scheme/typed host/effective portが同一と確認して、意図したcanonical absolute URLを保持する。path/query/fragmentだけをserializeした最終hrefにも、leading slashが正確に1個であることを再検査し、canonical network-relative表現を拒否する。その最終hrefを同じ固定baseへ再解決し、origin（scheme/typed host/effective port）と意図したcanonical URLのpath/query/fragment（空値と欠落も含む）が一致すると確認してからNavigationUrlを作る。不一致はError::Invalid。この最終検査はHTML attribute encoding前に行い、encoding後も既存output budgetを検査する。URIのpercent encodingとHTML attribute encodingを分離する。query/fragmentは許すがHTML要素id生成は初版に含めない。

このhref制約はserver DNS/socket接続の許可、SSRF special-address分類、endpointの認可、HTTP redirectやLocation等による全navigationの制御を意味しない。browser oracleでは、正規化後にnetwork-relativeとなる表現を拒否することと、許す最終hrefのbrowser destinationが意図したcanonical origin/path/query/fragmentと一致することを確認する。固定documentにbase要素を入れず、CSP `base-uri 'none'` を補助にする。SSRF用Targetと相互cast/共通unchecked URL wrapperを設けない。

base main/SF05にはurl/idnaが無かった。exact URL2.5.8 default/std→IDNA1.1.0 compiled_data→ICU2.3を6ef4d21で[開発用採用](sf04-url-adoption.md)した。review済みlockは25追加/置換0、source/checksum/declared licenseを固定し、宣言最大MSRV1.88に対し実toolchainは1.99のみ。URL2.5.8 [parser](https://docs.rs/crate/url/2.5.8/source/src/parser.rs)のC0/space trim、tab/newline無視、空userinfoのEmbeddedCredentialsと[idna](https://docs.rs/crate/idna/1.1.0/source/src/lib.rs)のDnsLength::Ignoreから、strict raw/credential/長さ検査を実装側で保持する。

古い2.2.x support tableはsupport/unsupported双方の証明ではなく、Nagi maintainerが更新/現policy追跡を所有する。fixed RustSecDBのaffected0 metadata比較は正式audit/安全承認ではない。ICU4X_DATA_DIR absent、MIT選択とUnicode/IBM原文notices、現feature union/最低toolchain/4OS/browserと生成app/CIの残受入条件を開発用判断へ分離する。
- `url 2.5.8` archive SHA-256: `ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed`; [registry archive](https://static.crates.io/crates/url/url-2.5.8.crate).
- `idna 1.1.0` archive SHA-256: `3b0875f23caa03898994f6ddc501886a45c7d3d62d04d2d90788d47be1b1e4de`; [registry archive](https://static.crates.io/crates/idna/idna-1.1.0.crate).

## 有限budgetとprofile — SF04-C05

| Budget | Default / hard ceiling | Accounting |
|---|---|---|
| output_bytes | 65,536 | Encoded UTF-8, markup + document wrapper included |
| input_bytes | 16,384 | Aggregate raw UTF-8 text/title/attribute/URL payload |
| nodes | 1,024 | Text slots, elements, seven fixed document nodes |
| depth | 16 | Maximum structural nesting, document wrapper included |
| urls | 16 | Number of href slots |
| url_bytes | 2,048 | Each raw and canonical URL; base origin also bounded |
| attributes per element | 2 | Optional title and href only |

`limits`はhard ceiling以下へ下げるだけ。output/input/nodes/depth/url_bytesは正、urlsだけ0を許しlinksを禁止できる。負値/上限超過はInvalid。policyのbase originはurl_bytesで別にstartup制限し、各fragmentのinput_bytesに繰り返し加算しない。すべての資源はcanonical base＋全limitsの同一profileを私有に持ち、別profileとのhref/element/join/copy/documentはInvalid。同値profileは別policy値でも同じとみなす。origin/予算を合成で拡大しない。

input_bytesはtext/title/URLのraw入力summaryを合算し、copyしたfragmentをjoinした分も二度数える。attrs/NavigationUrlは生成時に制限し、親elementへ渡すとそのsummaryを一度加算する。URLのrawとcanonicalの双方はurl_bytes以内、そのHTML escape後byteはoutput_bytesに含む。outputは最終UTF-8 bytesで、title/markup/wrapperを含む。checked arithmeticで加算とencoded lengthを先に検査してからbufferを伸ばす。

text slotは空でも1 node、empty fragmentは0、elementは1＋children。documentはdoctype/html/head/meta/title/title text/bodyの固定7 node＋body。text depth 0、BR depth 1、element depth children＋1、joinはmax。document depthはmax(3, body depth＋2)。全文の出力制限はdocumentで再検査し、wrapper分を省略しない。深さによらず無制限recursion/中間AST/vectorを作らない実装を選ぶ。

これは一つのvalue/responseの有限output/input/node予算であり、保存した多数のvalue/Task/shared値の総量、allocator RSS、総request CPU/HTTP期限を保証しない。同期builderの入力workは上記有限bytes/nodesに制限されるが、non-yielding中の強制取消を追加しない。OOM/panic/Task取消の既存境界も変更しない。

## Response/finalizer — SF04-C06

固定main3bのResponseはprivate status/headers/buffered Bytesだけだった。C06 runtime sliceはprivate BodyKind `Empty/Text/Binary/Json/Html`、CSPとmanaged MIMEを実装し、`html_response`だけが検査済みHtmlDocumentをconsumeしてHtmlを設定する。text/bytes/jsonのfixed MIME、nosniff、HEAD/204/205/304、reserved header拒否を維持する。対象actual unitの結果は限定した検証範囲として記録し、ce2ba17の独立C06実装reviewには修正必須指摘が無い。

finalizerがHtmlの `text/html; charset=utf-8` と他kindの既存MIME、全応答のnosniffを所有する。bytes/JSON/text/mapperを手動headerでHTML/JS/SVGへ変えられない。標準Rustのunchecked factory、pub fields、kind setterやconversionでも迂回できないことをnative compile-failで検査する。これはdownload後の内容や任意secretの安全性を保証しない。

全標準応答（typed HTML、text/JSON/bytes、mapper、builtin denial/error、HEAD、204/205/304）に次の固定CSPを出す契約。nonce、report endpoints、unsafe-inline、script/style source拡張は別契約へ分離する。

`default-src 'none'; script-src 'none'; style-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'`

Content-Type/CSP/nosniff/CORS/session等の現reserved集合を維持し、HTMLにはContent-Encodingもmanagedとしてappendを拒否、finalizerで再確認する契約。固定main3bのappend_headerはContent-Encodingを予約していなかったため、**nonHTML応答の既存挙動はこの契約では変えない**。全bodyへのglobal拒否を採るなら互換性判断を別記する。partial streaming/compression用HTML factoryはない。

C06の内部不変条件処理はrootが通常の実装判断として固定した。標準public APIでは作れないHTML Content-Encodingが私有stateにあれば、元body/headersを破棄し、固定`internal error`の500textへ置換して同じmanaged finalizerを通す。値をechoせず、新public fallible APIやnonHTMLのglobal拒否を増やさない。HtmlDocumentの消費bridgeはcrate-privateで、public raw再構築入口ではない。

HEADでもtyped document構築と予算検査を行い、その後bodyを空にしてrepresentation Content-Lengthを保持する。204/205/304はbodyとContent-Type/Content-Encoding/framingを除き、205はContent-Length 0という現挙動を維持する。CSP/nosniffはbody無しでも残す契約。HTML禁止statusを新checker規則で増やさない。

handler/mapperがtyped responseを返す場合も同じfinalizerを通る。builtin early errorは安全なtext kindで同じmanaged headerへ通す。builderのinvalid input/profile/構造/予算超過は既存Error::Invalid、検出したfallible allocation等の内部失敗はError::Internal、bounded static messageで入力をechoしない。process OOMの回復保証はしない。通常Result ErrをTask Failure/panic/取消に変えない。panic/mapper panic/期限/取消/応答開始後の限界は既存HTTP/Task契約を維持する。

## 移行と同等業務app — SF04-C08

raw `Html`/`html`/`http.html` はD1の削除対象で、現mainのraw http.html tombstone拒否を維持し、canonical tombstoneと「SF04 typed APIへ移行」の既存診断を維持する。import alias/function value/saved Low/manual Low/native替換で旧入口を復活させない。機能未定義の新APIを呼んで出るunresolved/parse errorをSF04のRED成功と数えない。

旧raw alias移行coverageのlocal `40877b3` は別worktreeのtest-only38行で、mainに統合済みではない。後で必要なら明示cherry-pickして独立結果/commitを記録し、ここでは再追加しない。

同等業務appの推薦oracleは既存policy必須Appの一覧/詳細ページ。動的業務title/descriptionをtextへ、任意quoted title、一覧のUL/LI、詳細への `NavigationUrl` `/items/7?view=details` をAへ、typed documentを `http.html_response` へ移す。既存public/protected routeとGrant照合/認可、業務status/Result mappingを維持する。High、saved Low、handwritten Lowの同じappでwireとbrowser DOMの一覧/詳細内容・リンクを確認する。plain text/JSONへの置換だけを同等HTML UIと数えない。

script/style/forms等を必要とする旧任意ページはこのsubsetと同等にならない。保証範囲に合わせてinert一覧/詳細へ再設計するか、別trusted host境界の選択が必要で、unchecked互換shimは作らない。業務app/DOM oracleは後続であり、runtime coreの限定検証だけを実app完了としない。

## Source根拠・今回の検証範囲

既存の二索引（SF04 source inventory/API-pattern inventory、親がGit object照合した2snapshot36hash）を引き継いだ。主要引用は固定mainの次の経路。SF05ではSQL差分で一部行番号が動くためmainを基準とする。

- [RFC](rfc.md)127–135、[D1移行](decisions-and-migration.md)13–21、[計画](implementation-plan.md)SF04行と63行：採用境界と未来acceptance。
- [registry](../../../compiler/src/stdlib.rs)275–303/450–591/711–730/1183–1193：resource flags、Passing、borrow_ownerとResponse/owned transform。
- [checked plan](../../../compiler/src/check/checked.rs)580–650/681–715と[emitter](../../../compiler/src/emit.rs)629–687：canonical operation/constant/fieldとsealed plan。emitterで名寄せした別checkerを作らない。
- [runtime finalizer](../../../runtime/src/http_server.rs)固定main3bの327–483、[Error](../../../runtime/src/lib.rs)29–77：mainではBodyKind/CSPが無く、既存MIME/header/HEAD/empty statusと通常Errorが基礎。現在C06 sliceは上記BodyKind/CSPを追加した。
- [独立metadata oracle](../../../compiler/tests/resource_contract_characterization.rs)349–405/532–605/686–780、[native app](../../../compiler/tests/security_sf01_native.rs)39–44/109–161、[header tests](../../../runtime/src/http_server/tests.rs)441–484：実装接続候補でありHTML検証結果ではない。

初回c5acf7b/R01修正1a0635dはDocs候補として保持し、独立recheck閉鎖後に6ef4d21で契約freeze/開発用依存採用を保存した。core独立reviewはdfbca0aについて必須指摘なし。後続actual Cargoでは公開core API1件、現在C06では公開factory API1件/unit9件/core20件/既存HTTP4件、同一binaryの全runtime260件pass/既存ignore1件を限定範囲の結果として記録する。以前の容量guard/失敗記録とHTTP ConnectionResetをこの成功で閉鎖しない。C06独立reviewとcompiler M01の限定到達点は後節へ記録する。SF04 nativeアプリ/4OS/browser/TLS/全feature acceptanceは未確認。

## 2026-10-10 契約freeze / Contract freeze

R01の最終href修正は独立recheckでDocs契約について閉鎖済み。rootの指示でC01–C08をfreezeし、URL2.5.8を[開発用採用判断](sf04-url-adoption.md)の条件下で採用する。初回候補/レビューを保持し、runtime sliceと後続compiler/finalizerの実装・検証結果を分ける。

## Compiler milestone M01（全feature acceptanceではない）

canonical `std.html` の6 resource/12操作と `std.http.server.html_response` をcompiler正本registryへ接続した。C01–C08の署名/Passing/borrow_owner/flags/7budgetを変えず、既存checkerのcanonical resource hintsと結果型、checked facts/sealed native-call planを使用する。cross-module document引数を利用者の同名型で代用しない。汎用copyはowned nested HTMLを既存payload遍歴で拒否し、shared handleのcopyと明示 `copy_fragment` を区別する。新keyword/独立checker/unchecked HTML入口は追加しない。

先行actual Cargoは未登録module assertion REDを観測し、その後対象5 test成功。最終追加検査のCargoはdisk保全guardで停止した（exit−15、検査未実行）。保存したactual unitをrootが後続の別stageで直接実行し、HTML6/同一手書き全inventory4/全文golden4/capability9と全unit138成功を確認した。原guard-stopをCargo成功へ読み替えない。一時的な全inventoryのunit登録は除去し、本来integration登録を二重化しない。原Cargoのguard-stopと、後続の保存unit直接実行は別の結果として扱う。

High/保存Low/独立手書きLowのcheckとRust生成、canonical名/所有権/拒否主行の有限検査であり、SF04 native app実行、9生成Rust出力snapshot、native trait compile-fail全表、本来integration Cargo、全workspace/Clippy/4OS/browser/TLS、公開Docs/同等業務app完成は未確認。既存小Rust native unit1はSF04 appの証明ではない。M01の独立review（report SHA-256 `4308f7ec4467343c8b9d080b520b5a06cb31449485cd6b4f7a59585f0eddea28`）は必須指摘なし。これは実装reviewであり、列挙した未実施acceptanceは閉じない。

## Compiler/native milestones M02–M03（限定検証）

M01の当時未確認範囲のうち、M02は現sourceの通常Cargo compiler build、High/保存Low/独立手書Lowの通常CLI check/lower、未編集Rust9出力、一覧/詳細mainのdirect native3build/runを保存し、その後の独立reviewで必須指摘無し（report SHA-256 `20bd9f3be0d57ee2b1d244c91493978c4aa8d858c8bda954547e4fb7e56ea99e`）となった。原Cargo guard-stopを成功へ書き換えず、runtime ef/current compiler libraryや原artifactを保全した。

M03は[永続harness](../../../compiler/tests/security_sf04.rs)の3業務正例と1checker負例群を追加する。親が実行前に手書き固定した386byte [一覧/詳細body](../../../compiler/tests/fixtures/security-sf04/expected-list-detail.html)をrenderer結果から生成せず比較する。rename例はNagiがpolicy/text/documentを構築してowned factoryへ渡し、同名利用者型のvalueも確認する。std.httpのみの例は通常textと同名型を確認する。各正例のHigh/保存Low/手書Low計9件は、現compiler正本から生成した未編集root Rust libraryと別host oracleで実build/runし、wholebody/statusを確認した。wire managed header/finalizerやmapper dispatcherをこのbody検査の証明にしない。

永続negative群は現compiler libraryへdirect linkして6case×3構文の18拒否（parse/load成功後のchecker理由と実入力主行）を確認した。通常 `cargo test --locked -p nagic --test security_sf04` とnested native Cargoの3正例は容量/feature graph見積りで未起動であり、CIのexact4件登録を実行成功としない。既存native_tripleにはadapter/assertions追加前のraw生成Rust保存だけを追加し、timeout/依存/assert/並列は維持する。通常release CLI/native Cargo app、native trait全表、mapper/実wire、全workspace/Clippy/4OS/browser/TLS/MSRV/URL性能/公開Docs/配布acceptanceは未確認。M03のsaved Low独立recheck（report SHA-256 `abb70e5d54c816b51ee7a538be3b4a542f7ad231b6f1d8593144e61b2546247f`）は必須指摘なし。C01–C08/API/flags/budget/言語意味論は不変。
