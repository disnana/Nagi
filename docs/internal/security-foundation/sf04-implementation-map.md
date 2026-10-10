# SF04: 実装/RED対応（freeze済み・開発中）

[公開契約](sf04-contract.md) · [English map](sf04-implementation-map.en.md) · [計画](implementation-plan.md)

2026-10-10 UTC。契約freeze後の開発中。固定main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`。未来の実装/テスト計画と後述する限定runtime core結果を区別する。全feature・native HTTP/browser/CI acceptanceの記録ではない。SF01が直接依存であり、未merged SF05やreader/editor作業をこのbranchへ混ぜない。

## 採用と実装の順序

1. rootと独立High reviewerがSF04-C01–C08をreviewし、通常のRFC具体化として採用する署名/subset/limits/URL/CSP/flagsを固定する。新型の通常API名/数値/許可subsetは既存RFCの具体化。新keyword/effect/typechecker、Task/move/Grant/Txの意味変更、安全境界の弱化、未採用unchecked互換API等は別の公開意味論変更であり、このreviewだけで確定しない。必要なら理由/影響を示してユーザー判断へ戻す。D1–D3の既に採用したbreaking migrationを再承認待ちにしない。
2. URL依存を使うならexact SF04 graph/source/license/MSRV/advisory/targetの採用条件を埋め、dependency/library採用結果を別に記録する。dependency consumer compileだけでrenderer安全性を主張しない。
3. 固定契約に基づく小さいRED oracleを保存し、failure stage/expected diagnostic/source位置を確認する。まだ登録のないstubのunresolved/parse errorはRED根拠にしない。typed API到達とoracleの意図したfailを区別できない場合はStopしてharnessを直す。
4. canonical metadata→checked facts/sealed plan→runtime opaque renderer→finalizerを小分けに実装する。独立手書きmetadata期待、High/Low/nativeの一致、実DOMを確認し、feature GREENと環境failを分ける。
5. 独立review、必要回帰、4 OS/native/controlled TLS/browser oracleを終えてからacceptanceとする。未実行browserをunit/mocksで置換しない。公開Docs/AI examples/日英も最後の採用APIへ同期する。

1は6ef4d21でfreeze済み、2は開発用採用と残受入条件を保存済み。API未定義stageとmodule behaviorを保持し、後続actual Cargoの公開core API1件を確認した。C06は既存API4件のcompiled CSP REDからruntime finalizerへ進み、現在public factory API1件/対象unit9件と同一binaryのcore20件/既存HTTP4件/full runtime260pass/1既存ignoreを記録した。C06独立reviewとcompiler M01の限定到達点は後節へ記録する。native SF04 app/4OS/browser acceptanceは後続。

## 接続点と有限oracle

| ID/対象 | 実装接続先 | Positive oracle | Negative/boundary oracle・拒否段階 |
|---|---|---|---|
| SF04-C01 canonical resources | compiler/src/stdlib.rs Resource/StandardModule/ResourceContract、独立resource_contract_characterization期待表 | 六resourceの完全inventory/flags、全operation signature/Passing/borrow_owner、Tag constantsとcanonical native pathsを手書き期待で検査 | user型の同名spoof、private fields、wrong resource/context、Copy/Serde/Debug派生をchecker元位置で拒否。storage/shared/cross-taskを受理した場合はnative trait/build成功 |
| SF04-C01 sealed lowering | compiler/src/check.rs、check/checked.rs、emit.rs既存plan経路 | 十三operationをHigh check→generated Low再parse/check→Rust build、saved Low/manual Lowで同じownership | moved attrs/fragment/document再利用、borrow後owner破棄、関数alias/Result nesting/loopを既存checkerで拒否。Nagi受理後のRust move/lifetime拒否はP1でありGREENにしない |
| SF04-C02 static subset | runtime/src/html.rs、closed HtmlTag metadata/constant table | title/hrefの順序、BR void、UL/LIとDIV/phrasing、固定document wrapperをexact bytes/DOM treeで確認 | dynamic/unsupported tag/attrは公開API/型で入口無し、nested A、list直下text/orphan LI、phrasing直下flow、BR非emptyはError::Invalid。metadataへの任意名registerで迂回させない |
| SF04-C03 contexts | 私有text/quoted attr/document-title encoder | 小さいUnicode/五文字/TAB/LF/CR、literal entity text、fragment join/copy後DOM logical値一致 | NUL/禁止controlsはInvalid、str/bytes→fragment/NavigationUrl castはcompiler/native consumer拒否。componentの再escape・markup解釈はDOM/byte oracleでfail |
| SF04-C04 navigation | 私有bounded URL adapter、HtmlPolicy/NavigationUrl | canonical HTTPS baseのIDNA/IPv4/IPv6、default port、root-relative query/fragment、最終hrefの単一leading slash/固定base再解決/意図したcanonical URL一致、href HTML encoding | 正規化後のnetwork-relative表現と再解釈でorigin/path/query/fragmentが変わる表現、absolute/network-relative/backslash/control/space、userinfo/root dot/nonroot base、異profileはInvalid。negativeはparse-only、DNS/第三者への接続はしない |
| SF04-C05 budgets | 私有summary/checked arithmetic/preflight buffer sizing | 小profileへlimitsを下げ、小inputで各上限−1/ちょうど/＋1、wrapper/title/URL/copy/node/depthの独立expected計算 | quota reset/overflow、profile混在、copy後joinの二重計上、0/負値/上限超過はInvalid。巨大入力/枯渇は作らない。native allocation測定は有限fixture条件を残す |
| SF04-C06 body classification | runtime/src/http_server.rs Response private BodyKind/representation/finalizer、stdlib HttpHtmlResponse | 小owned responseのHTML MIME/CSP/nosniff、text/bytes/json MIME不変、mapper/builtin early error、HEAD/204/205/304 | bytes/header/manual Rust conversionでactive MIMEに変えられない。全reserved fieldsとHTML Content-Encodingを拒否。nonHTML Content-Encoding現挙動を比較し意図せず変更しない |
| SF04-C06 standard Rust boundary | runtime public exports/private fields/trait implementations | &str/owned resourceの同じ検査、opaqueデータのSend/Syncをcontractと比較 | from_raw/unchecked/Deserialize/Display/Clone/private field/BodyKind setter/HTML MIME overrideのcompile-fail。カスタムtrusted Rustをstd APIと混同しない |
| SF04-C08 raw migration | 既存Operation::Html tombstone/checker診断、error_routes、別40877b3 coverage | raw移行後の実typed一覧/詳細appがHigh/Low/native/browserで同等業務内容とstatusを保持 | direct/import alias/function value/saved/manual Low/native_low/@replaceからraw旧入口を復活させない。parse failure/ICE/無関係なfailureは移行成功ではない |

新keyword/effect/独立solverは追加しない。viewはReference・所有戻りはborrow_owner=Noneという既存registryの一経路で扱い、emitterの名前リストでmoveを再実装しない。native/@replaceでも標準Rustのchecked factoryだけが型に合うことを検査する。任意custom Rustの権限をsandbox保証として宣伝しない。

## 小さいapp/transport/browser oracleの具体化

固定署名の採用後に一つのowned一覧/詳細fixtureを作る。最小内容は業務title、Unicodeを含むitem説明、任意title属性、UL/LI、`/items/7?view=details` のA。既存policy-required Appのpublicまたはprotected routeと元のGrant/業務Errを維持する。今はsource fixtureを書かず、このsemantic planを共有する。

| 観測層 | 記録する結果 | 他層を代替しない境界 |
|---|---|---|
| hand-written metadata | resource/operation集合、署名/flags/Passing/borrow_ownerの独立期待 | registry自身から期待を生成しない |
| High/saved/manual Low | parse/check、移行/ownership元位置、generated Rust build、同app実行 | check成功だけをRust/native成功にしない。別API未定義をREDにしない |
| runtime units | encoded bytes/summary/profile/低いbudgetの境界/managed header | browser解析の証拠ではない |
| native owned loopback | wire status/representation MIME/CSP/nosniff/HEAD/empty status、handler/mapper Error/panic/期限/取消/capacity解放 | 新typed部分のoracleを既存SF01成功から推測しない。non-yielding/response開始後の限界は残す |
| controlled TLS/browser | Chromium/Firefox/WebKitのversionとOS、document構造/text/quoted属性、最終hrefのdestinationと意図したcanonical origin/path/query/fragment一致、正規化後network-relative表現の拒否、HTTP actualheaders | mocks・HTML文字列grepを実DOM/TLSの代用にしない。無害な構造fixtureのみ、script execution/攻撃payloadは不要 |
| four native targets | Linux、Windows、macOS ARM、macOS Intelの新dependency/build/runtime | 過去4OS CI/harness登録だけでは新feature検証完了でない |

text/attribute/URLの不正例は所有する小fixtureで拒否結果を観測し、第三者scan/実外向request、巨大input、PoCや資源枯渇を行わない。CSPは固定directiveのheader oracleで確認し、script/style executionが「止まった」ことをencoder証明にしない。

## 公開Docsへの後続同期

採用/実装時にdocs/http-server.mdと英語版、resource/operations reference、AI向けHTML移行例、既存list/detail業務例を更新する。旧HTTP operation importやResponse body/headerのborrow/owned意味を維持し、unsupported forms/style/scriptを透明に示す。現SF01 migration docをtyped実装完了と誤記しない。

開発記録では、R01のDocs閉鎖、6ef4d21の事前freeze/開発用採用、core moduleのcompiled RED/GREEN、actual Cargoの容量stopを別の結果として扱う。metadata/finalizer/High/saved/manual Low/native HTTP/browser/4OS acceptanceは未完。契約採用と各層test結果を混同しない。

## Compiler M01の到達点と次の検査

6 resource/12 html操作/1 HTTPfactoryと12 Tag constantsをcompiler正本metadataへ追加。canonical hints/結果型は既存standard-call checker、Passing/move/viewは既存checked facts、native pathはsealed planの責務とする。非Clone HTMLは既存purpose別payload walkでowned wrapperを拒否しshared handleは除外する。独立の全inventoryを44 resource/95 operation/12定数集合へ拡張し、2 HTTP全文goldenには新factoryのDefId/bindingとRust reexportだけを追加した（旧本文/既存metadata順は不変）。

先行metadata assertion RED、対象5件のactual Cargo成功、最終Cargo guard-stop、後続actual binary直接23件および全unit138件の成功は別の結果である。High/saved/hand Lowは13操作・renamed imports・同名user型・HTTPだけのimportを検査し、High/saved negativesはcheck段階・理由・入力内主行を固定、manual Lowは同名document/owned nested Cloneの2 negativeを確認。一時integration表unit includeは最終sourceから除去した。

M01の独立review（report SHA-256 `4308f7ec4467343c8b9d080b520b5a06cb31449485cd6b4f7a59585f0eddea28`）は必須指摘なし。残る作業は9生成Rust出力の未編集snapshot保存、本来integration/CLI/Cargo/native SF04 appの有限実測、native opaque trait/visibility全表、同等業務app移行、公開Docs/AI例、4OS/TLS/browser/配布gate、全feature acceptance。未compileのsnapshot出力hook候補はsource milestoneから除外し、実snapshotに数えない。
