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

最新版Release APIの読戻しは[この記録](jetbrains-release-assets-readback-2026-10-08.json)を参照。公開済み`jetbrains-v0.1.1`は`nagi-jetbrains-IC-0.1.1.zip`と`nagi-jetbrains-PC-0.1.1.zip`を持つ。これらはその版の実在配布物として保持し、統合済みZIPと呼ばない。JetBrains Marketplaceの0.1.1も公開済みであり、新しいcommon ZIP candidateは公開済み配布物と混同しない。新しい版から`nagi-jetbrains-X.Y.Z.zip`ひとつを候補にし、IDEA/PyCharmのstable/EAP四経路すべてで同じsourceを検証できた後にだけ`release-jetbrains`配布artifactを作る。Marketplace登録と`com.disnana.nagi` IDを保持する。版は0.1.1のまま、tag・Release・Marketplace送信を行わない。

`TrustedProjects.isProjectTrusted`で既存の拒否側trust gateを維持し、`ProcessListener`へ移行する。`ProcessAdapter`も従来の`com.intellij.ide.impl.TrustedProjects.isTrusted` APIも新コードから参照しない。CIは一つの候補ZIPをartifactで渡し、IntelliJ IDEA/PyCharmのstable/EAPごとに分かれた四つのmatrix jobでIDE testとPlugin Verifierを実行する。`fail-fast: false`で他のmatrix jobを継続し、各jobのVerifierもIDE test失敗後に試す。失敗したjobや未実行のtargetを成功扱いせず、四つすべてが成功した後だけ元候補ZIPを`release-jetbrains` artifactとして再アップロードする。

stableの2025.1.1は既存JetBrains workflowとビルド説明が使ってきた対応対象であり、「最新IDE」として選んでいない。stableのIDE testはこの対応対象を維持し、Plugin VerifierではIDEA build 243（2024.3.7）とPyCharm build 243（2024.3.6）も確認する。EAPは両製品ともGradle IntelliJ Platform Pluginの`LATEST-EAP-SNAPSHOT`指定で解決する。各matrix jobはGradle cacheのSDK `product-info.json`を読み、実際に解決した版・build番号と元JSONを検証report artifactへ保存する。`LATEST-EAP-SNAPSHOT`という指定だけを解決済みのIDE版として扱わない。

JetBrains IntelliJ Community source snapshot `f793c25de80113f355c0041aec24cd14a6a5174c`は[`TrustedProjects.isProjectTrusted`](https://github.com/JetBrains/intellij-community/blob/f793c25de80113f355c0041aec24cd14a6a5174c/platform/platform-impl/src/com/intellij/ide/trustedProjects/TrustedProjects.kt)を定義する。旧[`ProcessAdapter`](https://github.com/JetBrains/intellij-community/blob/f793c25de80113f355c0041aec24cd14a6a5174c/platform/util/src/com/intellij/execution/process/ProcessAdapter.java)は`@Deprecated`で「ProcessListenerを直接使う」と記載され、[`ProcessListener`](https://github.com/JetBrains/intellij-community/blob/f793c25de80113f355c0041aec24cd14a6a5174c/platform/util/src/com/intellij/execution/process/ProcessListener.java)の`processTerminated`はdefault methodである。これらsource確認は、選択SDKに対するPlugin Verifier完了の代替ではない。

同じZIPを使うため、Gradle IntelliJ Platform Plugin 2.14.0の[`VerifyPluginTask.archiveFile`と`freeArgs`](https://github.com/JetBrains/intellij-platform-gradle-plugin/blob/2.14.0/src/main/kotlin/org/jetbrains/intellij/platform/gradle/tasks/VerifyPluginTask.kt)を使う。2.14.0の一次sourceでは`archiveFile`が`BuildPluginTask.archiveFile`を既定値とし、`freeArgs`をPlugin Verifierへ渡す。安定IDEA向けに一度だけ生成・検査した`build/release-assets/nagi-jetbrains-X.Y.Z.zip`を四つの独立したverifier processに渡し、すべて成功した後で同じzipとchecksumをアップロードする。実Gradle/VerifierのCI結果が未読戻しの場合は、ローカルsource調査を実行結果の代替とはしない。

fe316f1時点の先行ローカル検査はRelease Python suite 116件、CI Python suite 60件、website生成98ページのlink・anchor・asset検査が成功した。現行のPublic Marketplace案内・description更新後もCI suite 62件、website 98ページ、JSON、`git diff --check`が成功した。`actionlint`はローカル環境にない。以前のGradle 9.0.0 wrapper実行は`services.gradle.org`への接続拒否でdistribution取得に失敗した。2.14.0に合わせたGradle 9.4.0のwrapperは、次のPR CIでGradle testとPlugin Verifierを確認する。

PR #102の独立source reviewではblocking findingはなく、README日英の検証範囲説明に対する非blocking指摘を反映した。最低対象のIDEA/PyCharmはPlugin VerifierのみでありIDE testの対象ではないこと、共通candidateは4経路の検証前に作り全成功後に配布artifactへ昇格することを明記した。レビュー時点では実SDKとPlugin VerifierのCI検査が必要であり、独立reviewをその実行成功とは扱わない。レビュー対象、日英Docs修正、先行するPython検査コマンドと出力excerptの由来は[検証記録](jetbrains-pr102-validation/provenance.json)に保存した。

## EAP module descriptorと実解決SDKの記録（2026-10-08）

修正前HEAD `fe316f118a06b5c3dfa6d486f3927d3e92b922a3` の[PR CI run 37772494079](https://github.com/disnana/Nagi/actions/runs/37772494079)では、IDEA/PyCharmの両EAP testが`module-descriptor.xml`のroot `module/namespace` attributeを`UnknownXmlFieldException`として拒否した。2.12.0へ上げた後の[run 37775493318](https://github.com/disnana/Nagi/actions/runs/37775493318)でroot側のattribute問題は解消したが、PyCharm EAPはdependency側の`module/namespace` attributeで失敗した。IDEA stable Plugin Verifierは別のAPI互換性問題として`TrustedProjects.isProjectTrusted(Project)`の欠落を検出した。このAPI修正と対応testは別担当が行い、この配布作業では触らない。

Gradle IntelliJ Platform Pluginは2.3.0から2.12.0を経て、dependency descriptor属性を読む最小の公式修正版2.14.0へ上げる。2.12.0はroot `ModuleDescriptor.namespace`/`visibility`を追加したが、dependencyの未知属性はまだ失敗する。2.13.0の[ModuleDescriptor test](https://github.com/JetBrains/intellij-platform-gradle-plugin/blob/2.13.0/src/test/kotlin/org/jetbrains/intellij/platform/gradle/models/ModuleDescriptorTest.kt)はroot namespaceだけを検査し、dependencyに追加属性を含めない。2.14.0の[公式changelog](https://github.com/JetBrains/intellij-platform-gradle-plugin/blob/2.14.0/CHANGELOG.md)はmodule descriptor parsingがadditional dependency attributesを読む修正を明記し、[回帰test](https://github.com/JetBrains/intellij-platform-gradle-plugin/blob/2.14.0/src/test/kotlin/org/jetbrains/intellij/platform/gradle/models/ModuleDescriptorTest.kt)は`namespace="jps"`のdependencyを含むdescriptorをdecodeして名前を確認する。build scriptは2.12で移行済みの[dependency extension API](https://github.com/JetBrains/intellij-platform-gradle-plugin/blob/2.14.0/src/main/kotlin/org/jetbrains/intellij/platform/gradle/extensions/IntelliJPlatformDependenciesExtension.kt)と[verification IDE API](https://github.com/JetBrains/intellij-platform-gradle-plugin/blob/2.14.0/src/main/kotlin/org/jetbrains/intellij/platform/gradle/extensions/IntelliJPlatformExtension.kt)を保つ。

JetBrains 2.14.0 releaseの[公式wrapper](https://github.com/JetBrains/intellij-platform-gradle-plugin/blob/2.14.0/gradle/wrapper/gradle-wrapper.properties)と公開module metadataはGradle 9.4.0を使っているため、このproject wrapperもGradle 9.4.0へ合わせる。binary distributionは[Gradle公式配布](https://services.gradle.org/distributions/gradle-9.4.0-bin.zip)から取得し、[公式SHA-256](https://services.gradle.org/distributions/gradle-9.4.0-bin.zip.sha256) `60ea723356d81263e8002fec0fcf9e2b0eee0c0850c7a3d7ab0a63f2ccc601f3`を固定する。

Plugin Verifierには、明示要求されたplugin display nameを維持したまま`TemplateWordInPluginName`だけを`-mute`する。deprecated/experimental/API互換性の診断はmuteしない。2.14.0の`VerifyPluginTask` sourceで`archiveFile`と`freeArgs`が存在することを確認し、同一候補ZIPの四経路検査を維持する。

Stable/EAP各matrix jobは検査終了後にも必ずGradle cacheの該当SDK `product-info.json`を探す。aliasと一致するSDK artifact directoryのJSONが一つの版/build/product identityに定まらない場合はjobを失敗させる。元JSON、SHA-256、要求alias、解決したversion/build number/product codeを`nagi-jetbrains-reports-<product>-<channel>` artifactへ保存し、step summaryにも実解決値を表示する。このworkflow変更のEAP実行結果は未確認であり、修正後のPlugin Verifier互換性成功を示すものではない。

## Marketplace listingとIDE内recommendation（2026-10-08）

正式公開先は既存[Nagi Marketplace page](https://plugins.jetbrains.com/plugin/34891-nagi)。最初のreadbackは[こちら](jetbrains-marketplace-readback-2026-10-08.json)に履歴として保存してあり、そのdate-only recordでは`hasUnapprovedUpdate=true`だった（当時の時刻は記録していない）。12:22:29 UTCに再取得した[現在のreadback](jetbrains-marketplace-current-readback-2026-10-08-122229Z.json)では`hasUnapprovedUpdate=false`となり、Marketplaceの現在のGetting Started HTMLはMarketplace直接導入とGitHub Releases ZIPの両方を記載する。安定版URLは`/versions/stable/1189472`、version 0.1.1である。APIの`approve=false`値はそのまま保存し、availabilityの判定には流用しない。最新のユーザー更新に従い0.1.1をPubliclyAvailableと案内し、公開済み版のMarketplace ID `com.disnana.nagi`を維持する。Marketplace WebUIのGetting Startedはユーザーが直接管理するため、このPRは原稿を複製・同期せず、自動更新もしない。

repositoryの[source descriptor](../../editors/jetbrains-nagi/src/main/resources/META-INF/plugin.xml)をMarketplace descriptionの正本として英語を先、日本語を後にし、機能、IntelliJ IDEA/PyCharm対応、Nagi compilerを別途インストールする要件を記載する。Marketplace Getting Startedの実表示で確認された`h3`、`h4`、`p`、`a`、`hr`だけを使い、GitHub Releasesリンクはコンパイラの取得先として示す。CIはXML parseとこのローカルtag/attribute whitelistを確認するだけで、Marketplace rendererとの一致は保証しない。Marketplace APIの現readbackは前の英語descriptionを返しているため、新しいXML descriptionがMarketplace上にrender済みとは主張しない。版・tag・Releaseを変更せず、Marketplace uploadも行わない。

JetBrainsの[Plugin recommendations](https://plugins.jetbrains.com/docs/marketplace/intellij-plugin-recommendations.html)文書を2026-10-08に読み、取得したHTMLのSHA-256 `486088f30b2e85b43c2a156bcae0639b46831c6a5ae38c6188af53f95776cb48`を照合した。文書はMarketplace approval後でもIDE内推薦は適切な文脈に限られ、JetBrainsが個別要件を追加または推薦を見送れると説明する。Feature Extractorはbytecodeを静的解析し、動的値では抽出結果が不完全になる場合がある。`com.intellij.fileType` extensionの全属性は解析対象で、bundled file typeと競合する場合は推薦対象にならない。

現在の[plugin.xml](../../editors/jetbrains-nagi/src/main/resources/META-INF/plugin.xml)はID `com.disnana.nagi`を維持し、High/Lowのfile typeを`com.intellij.fileType` extension pointで`nagi`と`low`の拡張子として静的登録している。記述形式は公式文書にある静的解析対象と一致するが、実際にIDEのcontextual recommendationへ掲載されることや、bundled file typeとの非競合は検査していないため、IDE内推薦があるとは保証しない。Run Configuration、Facet、その他の機能登録を追加する要件もこの文書からは導かない。このfeature-extraction文書は表示名lint`TemplateWordInPluginName`の理由付けとは別の根拠である。
