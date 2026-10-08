# JetBrains trust APIの互換性判断

2026-10-08。PR #102の最低対応build 243と、stable/EAPを同じZIPで検証する際に発見した実API差分。plugin IDと公開versionは変更しない。ユーザーは最低対応版を2025.1（build 251）へ上げる案を採用した。従来APIへのadapterは追加しない。

## 確認した根拠

- checks run 37775493318のIDEA stable job 113306804992で、最低IDE `IC-243.28141.18`に対する`TrustedProjects.isProjectTrusted(Project)`が未解決。Verifierは実行時`NoSuchMethodError`の可能性とInternal class利用を報告した。stable `IC-251.25410.109`では同じAPIが利用可能。
- [243の新namespace source](https://github.com/JetBrains/intellij-community/blob/idea/243.28141.18/platform/platform-impl/src/com/intellij/ide/trustedProjects/TrustedProjects.kt)はobject全体がInternalで、LocatedProjectのoverloadしかない。
- [243の従来source](https://github.com/JetBrains/intellij-community/blob/idea/243.28141.18/platform/platform-impl/src/com/intellij/ide/impl/TrustedProjects.kt)には`Project.isTrusted()`があるが、file全体がExperimental。Javaからの入口は`com.intellij.ide.impl.TrustedProjects.isTrusted(Project)`。
- [251のsource](https://github.com/JetBrains/intellij-community/blob/idea/251.25410.109/platform/platform-impl/src/com/intellij/ide/trustedProjects/TrustedProjects.kt)はpublicな`@JvmStatic isProjectTrusted(Project)`を追加している。現代のSDK sourceだけで243にも存在すると判断してはいけない。

## 選択肢

| 案 | trust動作と保証 | 互換性・残る制約 |
|---|---|---|
| 限定互換adapter | 251以降は新public API、243だけ従来の公式trust入口へ委譲。SDK差分を隠さず、取得不能・呼出し例外は実行拒否。falseの結果から別APIへ再試行しない。trust検査は保存・queue・processより前に置く | 243の更新経路を維持。243の入口はExperimentalのままであり、全SDKで安定APIのみになったとは宣言しない。共通ZIPで存在しないmethodへstatic参照を作らないため、固定class/methodの限定lookupが必要。静的Verifierの警告が出ないことはこの旧API利用がなくなった証明ではない |
| 最低251 | 新public APIを直接使い、`sinceBuild`と最低Verifier対象を251へ更新 | 243利用者は新versionへ更新できず、既存0.1.1を使い続ける。ID・Marketplaceページは維持するが、現在の最低対応範囲を狭める変更となる |

adapterを採る場合は、実最低SDKでtrusted/untrusted双方を実行し、未信頼時のcommand呼出0・未保存内容維持を確認する。stable/EAPでも独立実行し、lookup失敗・例外・falseからのfallback拒否を小さなテストで固定する。テストでtrustを無条件trueにするhelper、独自trust store、警告の全体muteは追加しない。

ユーザーの回答「最低対応版を2025.1へ上げる」を正本として、`sinceBuild=251`と両製品の最低Verifier対象2025.1を揃えた。stable IDE testは2025.1.1、EAPは独立して継続する。新APIは保存・queue・processの前に呼ぶ既存gateを維持し、旧APIへのfallbackを追加しない。2024.3では公開済み0.1.1を利用できるが次版へ更新できず、IDE更新が移行方法となる。これは未リリースの対応範囲変更であり、plugin ID・既存Marketplaceページ・公開済みassetは変更しない。修正後の実SDK/Verifier成功は新HEADのCIで別に確認する。
