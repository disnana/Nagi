# JetBrains統合PR #102の引継ぎ

記録日: 2026-10-09 JST（観測時刻はUTCを併記する）。対象は[PR #102](https://github.com/disnana/Nagi/pull/102)。mainの#101 mergeを取り込み、baseは `c3e5fc7a795d66c8f6408ac12b480230dd3a8973`。プラグイン版0.1.1、既存ID `com.disnana.nagi`、Marketplace34891を維持する。merge・版更新・tag・release・Marketplace uploadは未実施、別途明示承認が必要。

## 完成した設計と実装

- 共通source・version・一つのZIPをIDEA/PyCharmで使う。候補は最低対応stable IDEA/JDK21で一度生成する。
- IDEA/PyCharmのstable/EAPそれぞれで実IDE SDKのtestと、同じ候補ZIPのPlugin Verifierを実行する。四経路成功後に元の候補bytesを配布artifactへ昇格する。
- 次版最低対象はIDEA2025.1.1 build251.25410.109、PyCharm2025.1.1 build251.25410.122。2024.3と初期2025.1 build251.23774は対象外。公開済み0.1.1を維持するかIDEを更新する。旧experimental trust APIへのadapterは使わない。
- ProcessListenerと公開isProjectTrustedを使い、未信頼projectのコマンド実行を拒否する。compilerは別途install。
- Gradle wrapper9.4.0、公式IntelliJ Platform Gradle Plugin2.19.0。候補・stableはjavac21、現在のEAP SDKはjavac25。すべてAPI/bytecode release21を検査・記録する。
- Descriptionはplugin.xmlを正本として英語→日本語のHTML。Getting StartedはユーザーがMarketplace Web UIで管理し、原稿複製・同期・自動更新をしない。
- README・日英Docs・公式サイトに公開済みの入手先とVSIX/ZIPの手動installを統一。未来の共通ZIPは公開済みと案内しない。

## 検証結果

source `52177e848410a3800d3a9ea34327f7b2cd3ecc65`の[checks37802356248](https://github.com/disnana/Nagi/actions/runs/37802356248)は2026-10-08 16:13:17 UTCに成功。Linux全workspace・native/examples、Windows/Linux/macOS ARM/macOS Intel、VSIX、共通ZIP candidate/promotion、Ready to mergeはすべて成功。PRなのでpublish-releaseはskip。[website37802355936](https://github.com/disnana/Nagi/actions/runs/37802355936)も成功。

| SDK | 実build | javac | IDE tests | Plugin Verifier |
| --- | --- | --- | --- | --- |
| IDEA stable | 251.25410.109 | 21 | 40/40、skip0 | Compatible |
| PyCharm stable | 251.25410.122 | 21 | 40/40、skip0 | Compatible |
| IDEA EAP | 263.6259.32 | 25 | 40/40、skip0 | Compatible |
| PyCharm EAP | 263.6259.38 | 25 | 40/40、skip0 | Compatible |

Verifierのdeprecated/experimental/internal APIとincompatibilityの報告なし。source/target/releaseはすべて21。test-onlyのDisposer.isDisposed非推奨noteとCI actionのNode deprecationは別で、muteしていない。共通candidate artifact11560618617、配布artifact11560594871はともに57,310 bytes。CIは同じinner ZIP/checksumを再検査してpromotionした。artifact APIのouter digestはinner plugin ZIP digestではなく、直接downloadが403のためinner digestの独立読戻しは未確認。

[保存資料](../jetbrains-pr102-validation/final-2.19/README.md)に一次source・実SDK javap、RED/GREEN XML、独立review、CI rawlogs、source hashとmetadataを同梱する。結果追記と最後の負例強化を含むHEADは本文へ自己参照せず、PR #102の実際のHEAD/Checksを読戻す。未信頼fixtureにeligible local source・拒否dialogのassertも追加し、単独EAP再実行1/1成功。最終HEADの四SDK/4 OSはそのChecksを別に確認する。

先行REDでは、javac21によるEAP class69/65不一致、IDE home未指定、bundled plugin test classpath不足を別々に観測した。公式dependencyと明示toolchainで修正し、assertion/skip/警告muteで隠していない。2.19先行test40件は39成功1失敗。実SDK bytecodeがheadless/unit-testを既定でtrust済みにすることを確認し、test JVMだけでshortcutを無効化。実際のuntrusted状態とaction直後の未保存を追加assertし、既存の待機後未保存/compiler未起動を維持した。ローカルIC EAP263.6259.32は40/40成功、skipなし。

独立Sol Highはclasspath/API/Java21/配布gateとtrust fixtureを確認しblockingなし。公式masterのKotlin sourceと実SDK bytecodeは別資料として記録し、同一revisionのsourceとは主張しない。

## 公開状態と限界

Marketplaceの公開済み0.1.1は[既存ページ](https://plugins.jetbrains.com/plugin/34891-nagi)と[承認済み版](https://plugins.jetbrains.com/plugin/34891-nagi/versions/stable/1189472)。GitHub Releasesには歴史的IC/PC ZIP二つがある。Nagi本体0.1.11、VS Code VSIX0.1.13の存在もreadback済み。このPRは将来の配布構成を変更し、公開assetを書き換えない。

stable検証は最低版2025.1.1であり最新stableではない。実SDK上のheadless fixtureは実行済みだが、desktop GUIでの手動操作、Marketplaceに新しいZIPをinstallする操作、Descriptionのbrowser rendering、Marketplace uploadは未確認。手動smokeは次版公開前にIDEA/PyCharmそれぞれで行う。

## 次に進む工程

型に基づく補完・定義ジャンプ・編集中診断は別PRにする。compilerを意味解析の正本とし、JetBrains側に型checkerを複製しない。High/Low、binding identity/shadowing、import、move/viewを考慮する。古い解析結果を新しいdocumentへ適用しない。persistent process・debounce・cancel・cache、EDT非blocking、trust拒否と処理終了を検証する。

既存の未公開作業は `/workspace/Nagi-jetbrains-assistance`（`feat/jetbrains-semantic-assistance`）に保存されている。初めに現在のmainと#102の実際のmerge状態を読み戻し、そのbaseへ更新する。compiler/Javaの変更を棚卸しし、再実装や同file同時編集を避ける。共有Cargo cacheのbinaryを対象commitのbinaryと決めつけず、exact sourceからbuildして正負・未完成source・cancel・実測を再確認する。user-record fieldの宣言navigationなどの未対応を明示する。

PR #102の自動merge・公開は行わない。追加の意味解析機能をこの配布統合PRへ入れない。
