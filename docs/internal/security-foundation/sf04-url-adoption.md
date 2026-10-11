# SF04 URL依存: 開発用採用とbuild input条件

[English](sf04-url-adoption.en.md) · [契約](sf04-contract.md)

2026-10-10 UTC。実装開始の開発用採用判断であり、SF04全acceptance/配布/安全証明ではない。rootの継続指示により `url = { version = "=2.5.8", default-features = true }` をruntimeへ追加し、review済みexact lock（SHA-256 `dc308b07a5ebfe3920ff552e8f44c7b2dbc8c54608cfe3aafb54de6f7f996892`）を使う。C01–C08/最終href契約は変えない。

## 判断の根拠

独立review `SF04-URL-DEPENDENCY-R01`（SHA-256 `38528fb4a847e46b06ab875be656f148fd492563c72e16054d624f4810bc7961`）に必須factual修正/実証されたversion/graph blockerは無い。既存25package追加、削除/置換0。URL default/std→IDNA1.1.0 alloc/std/compiled_data→ICU2.3 compiled_data。existing workspace unionではsyn foldだけ追加。source archive/checksum/manifest/license textsは固定研究と独立reviewで照合済み。metadata/treeは4target選択graphであり4OS buildではない。

runtime graph最大宣言MSRVは1.88（baseline1.85）、URL1.63/IDNA1.57だけで決めない。実行toolchainは1.99のみ、1.88未実測。Fastの通常URL consumerはoffline4.008s/exit0、四つのexample.invalid通常URLのorigin/path/query/fragment/empty-vs-absent/IDNAを確認しただけ。renderer/R01最終href政策/browser/HTTP/native app/4OS/MSRVの証拠ではない。

固定公式RustSecDB `7eebec69c352c7191b1f13eb95dd510eeca5d1de`、2026-10-09時点、archive SHA-256 `c71855c79d75ef2133c703d7034037b4674975690bed0594857ddbe629faf469` のversion-range metadata比較でcandidate affected0。patched-range/informationalと未確認reachability/alias/Git/path/feature/target/正式scanner policyを区別し、cargo-audit完了/安全承認と呼ばない。

URL/IDNAのpinnedとsaved current SECURITY.mdは2.2.x表のまま。これは2.5.8/1.1の現support保証でもunsupported証明でもない。保守support scopeはunknownとして採用する。更新責任はNagi maintainer: lock更新ごとにexact source/graph/license/現DBを確認し、upstream support情報を追跡して必要な更新/代替を実施する。release recency/yank=falseを保守保証にしない。

## ICU build input境界

追加2data packageのbuild.rs/lib.rsは `ICU4X_DATA_DIR` が存在すると（空値でも）外部data includeへ変わる。archive/lock pinだけではそのsource identityを固定できない。標準SF04検証の子processではこの変数をunsetとし、host/child presenceだけを記録し値/秘密を表示しない。review済みcompiled bundled dataを使い、custom dataは別source/hash/license review条件とする。

SF04 target CIではこの条件を明示env/input checkへ接続し、実build logへpresenceとbundled source identityを残す必要がある。generated appも同じruntime feature/lockを使うbuildで条件を適用・検証し、ambient overrideがある無記録buildを標準検証済みと呼ばない。現compiler全体のenv sanitize、任意host Rustの環境変更、暗黙のCI実装済み主張は行わない。対象sliceのbounded runnerだけへ条件を適用し、CI/generated-app経路接続は残る受入項目。

## 残acceptance/配布条件

- actual consuming workspace feature union/URL adapterとtyped rendererの小RED/GREENを別検証。
- intended minimum toolchain、Linux/Windows/macOS ARM/Intel build/runtime、実browser DOM/TLS/origin interpretationを実測。
- 新25packageのUnicode V3（IBM含むcopyright/attribution）、MIT、選択dual-license noticesを配布前にbundle。別担当のnotice原稿は本実装者が編集しない。全既存project notice監査は別範囲。
- 現maintenance evidence、advisory policy/date/lock、source/reachability/正式scannerの未確認を採用更新に残す。
- ICU override absentをCI/generated appで記録して確認するか、custom dataのsource/hash/licenseを別review。

開発用にresolver feasibleな固定候補を選ぶ理由は、独自URL grammarを避け、RFCのstrict raw→canonical final href→再解決→encodingの境界を一つのparserで実装できること。safeの一般宣言、無期限support、正式audit/全target成功の宣言はしない。

Dual-licenseはMITを選択し、各shipped MIT/Unicode V3/IBM copyright・permission・attribution/COPYRIGHT本文を保持する。notice bundleは別担当/rootが保存・配布gateへ接続する。
