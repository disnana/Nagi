# JetBrains Release接続の結果（2026-10-07）

この記録の当時の配布案（IC/PC別ZIP）は、2026-10-08の「共通ZIP移行」によって更新済み。現行契約は末尾の移行節を参照。

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

## 共通ZIP移行（2026-10-08）

最新版Release APIの読戻しは[この記録](jetbrains-release-assets-readback-2026-10-08.json)を参照。公開済み`jetbrains-v0.1.1`は`nagi-jetbrains-IC-0.1.1.zip`と`nagi-jetbrains-PC-0.1.1.zip`を持つ。これらはその版の実在配布物として保持し、統合済みZIPと呼ばない。新しい版から`nagi-jetbrains-X.Y.Z.zip`ひとつを候補にし、IDEA/PyCharmのstable/EAP四経路すべてで同じsourceを検証できた後にだけ`release-jetbrains`配布artifactを作る。Marketplace登録と`com.disnana.nagi` IDを保持し、review statusは公開中と誤記しない。版は0.1.1のまま、tag・Release・Marketplace送信を行わない。

`TrustedProjects.isProjectTrusted`で既存の拒否側trust gateを維持し、`ProcessListener`へ移行する。`ProcessAdapter`も従来の`com.intellij.ide.impl.TrustedProjects.isTrusted` APIも新コードから参照しない。CIは一つの候補ZIPをartifactで渡し、IntelliJ IDEA/PyCharmのstable/EAPごとに分かれた四つのmatrix jobでIDE testとPlugin Verifierを実行する。`fail-fast: false`で他のmatrix jobを継続し、各jobのVerifierもIDE test失敗後に試す。失敗したjobや未実行のtargetを成功扱いせず、四つすべてが成功した後だけ元候補ZIPを`release-jetbrains` artifactとして再アップロードする。

stableの2025.1.1は既存JetBrains workflowとビルド説明が使ってきた対応対象であり、「最新IDE」として選んでいない。stableのIDE testはこの対応対象を維持し、Plugin VerifierではIDEA build 243（2024.3.7）とPyCharm build 243（2024.3.6）も確認する。EAPは両製品ともGradle IntelliJ Platform Pluginの`LATEST-EAP-SNAPSHOT`指定で解決する。実際に解決されたIDEの版・build番号は、CI後に各matrix jobのGradle/Plugin Verifierログから読み取り、この記録へ追記する。

JetBrains IntelliJ Community source snapshot `f793c25de80113f355c0041aec24cd14a6a5174c`は[`TrustedProjects.isProjectTrusted`](https://github.com/JetBrains/intellij-community/blob/f793c25de80113f355c0041aec24cd14a6a5174c/platform/platform-impl/src/com/intellij/ide/trustedProjects/TrustedProjects.kt)を定義する。旧[`ProcessAdapter`](https://github.com/JetBrains/intellij-community/blob/f793c25de80113f355c0041aec24cd14a6a5174c/platform/util/src/com/intellij/execution/process/ProcessAdapter.java)は`@Deprecated`で「ProcessListenerを直接使う」と記載され、[`ProcessListener`](https://github.com/JetBrains/intellij-community/blob/f793c25de80113f355c0041aec24cd14a6a5174c/platform/util/src/com/intellij/execution/process/ProcessListener.java)の`processTerminated`はdefault methodである。これらsource確認は、選択SDKに対するPlugin Verifier完了の代替ではない。

同じZIPを使うため、Gradle IntelliJ Platform Plugin 2.3.0の[`VerifyPluginTask.archiveFile`](https://github.com/JetBrains/intellij-platform-gradle-plugin/blob/2.3.0/src/main/kotlin/org/jetbrains/intellij/platform/gradle/tasks/VerifyPluginTask.kt)を`-PverificationArchive`で上書きする。2.3.0の一次sourceには、この`RegularFileProperty`の既定値が`BuildPluginTask.archiveFile`で、VerifyPluginTaskがそのpathをPlugin Verifierへ渡す実装がある。安定IDEA向けに一度だけ生成・検査した`build/release-assets/nagi-jetbrains-X.Y.Z.zip`を四つの独立したverifier processに渡し、すべて成功した後で同じzipとchecksumをアップロードする。実Gradle/VerifierのCI結果が未読戻しの場合は、ローカルsource調査を実行結果の代替とはしない。

この移行後のローカル再検査ではRelease Python suite 116件、CI Python suite 60件が成功した。JetBrains workflowのYAML parse、`git diff --check`、website生成98ページのローカルlink・anchor・asset検査も成功した。`actionlint`はローカル環境にない。Gradle Wrapperはnetwork有効の追加権限でも`services.gradle.org`への接続拒否で配布物を取得できず、JetBrains Gradle testとPlugin Verifierは未実行である。CIの実IDE検査結果がそろうまではcommon candidateを公開用artifactへ進めない。
