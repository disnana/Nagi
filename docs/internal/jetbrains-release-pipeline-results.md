# JetBrains Release接続の結果（2026-10-07）

対象はmain `97b7242c2172c1d0c701a7b662294e4dbe268abf`からの独立したCI・配布処理の変更。Task/spawn、compiler/runtime、SQLite、公開APIは変更しない。プラグイン版は0.1.0を維持し、merge・tag・正式Release・Marketplace公開は行っていない。

作業中にユーザーがTask統合PR #95をmainへmergeしたため、最新main `611a9774a2104e62454339ce5bb5fe519ac68da4`へrebaseした。progressの競合は両方の工程記録を保持して解決した。Taskの実装・追加回帰・公開状態の修正を上書きしていない。JetBrainsは別の[PR #96](https://github.com/disnana/Nagi/pull/96)で確認する。

## 配布契約

- 今後`editors/jetbrains-nagi/build.gradle.kts`の版を増加させ、対応する`## JetBrains X.Y.Z`のCHANGELOG項目とともにmainへ反映すると、同じNagi checks runの必須検証成功後に`jetbrains-vX.Y.Z`を公開する。
- IDEA（IC）とPyCharm（PC）の双方で実コンパイラ連携テスト、buildPlugin、現行・最低対象IDEへのPlugin Verifierを実行する。生成ZIPのplugin JARから`META-INF/plugin.xml`を読み、IDと期待版を確認する。
- `nagi-jetbrains-IC-X.Y.Z.zip`・`nagi-jetbrains-PC-X.Y.Z.zip`と各SHA-256を、同一runのartifactから公開する。既存publisherのdraft・アップロード後の読戻し・不一致asset拒否・公開済みRelease不変性を維持する。JetBrains公開はNagiのlatestを置換しない。
- JetBrains単独版更新でLinux検査が計画上skippedの場合だけ、そのskipを許容する。Linux failure/cancelは許容せず、Nagi/VS Codeも同時に公開する場合はLinux successを要求する。両IDEの成功は常に必須。
- PR・main以外・手動実行・版を変えないpipeline追加は正式公開しない。release suiteはLinux jobから必須release-plan jobへ移し、JetBrains単独更新でも実行する。mainのIDE検査はreusable callerへ一本化し、別コミットの公開検査を取り消さない。

## 検証と独立レビュー

ローカルでRelease suite 113件、CI suite 59件、Python compile、両workflowのYAML parse、差分whitespace検査が成功した。CI suiteは実workflowの公開条件を限定文法で読み、手書きtruth tableでmain限定・版変更・各package・Linux skip例外・失敗と取消を確認する。publisherの検証にはFakeGitHubだけを使い、外部へのRelease/tag/asset書込みは行っていない。

Sol 6.1 Highの実装担当から独立したread-onlyレビューで、公開条件、同一run・両IDEの依存、初回pipeline追加と版更新の区別、draft再開、immutable Release、日英Docsとfixtureを確認し、未解決指摘は0。reviewer自身によるテスト再実行は行っていない。

実SDKでのGradle生成ZIPとPlugin VerifierはPR CIで確認する。最新HEAD・CI・artifactの確認結果はPR本文へ記録する。ローカルのunit成功をIDE検証や正式公開の成功と扱わない。完全なIDE画面操作とMarketplace公開はこの変更の検証範囲外。

最初のsource head `fcb0063c058139ba9908e18fb058e7a2c6dab298`の[checks run 37620375127](https://github.com/disnana/Nagi/actions/runs/37620375127)では、IDEA・PyCharmの各39テストがfailed/errors/skipped 0で成功した。実コンパイラ連携テストも各1件を実行し、両製品の現行・最低対象IDEのVerifierはCompatibleだった。現行IDEでdeprecated API 1件、両対象でexperimental API 2件の既存警告があり、警告なしとは報告しない。ZIP・reports artifactをダウンロードし、外側artifactのGitHub digest、ZIPとSHA-256、内側plugin.xmlのID/0.1.0版を照合した。この観測を最新rebase HEADのCI成功へ数えず、最新HEADはPRで別に読み戻す。

ローカル原ログは作業環境の`/workspace/nagi-jetbrains-release-2026-10-07/`へ保存した。website初回はMarkdown依存がないPython、その後は許可外の出力先指定で失敗し、既存venvと`build/`内の出力先で再検証した。これらを検査成功に数えない。
