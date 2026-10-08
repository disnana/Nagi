# Nagi for JetBrains

[English](README.en.md)

IntelliJ IDEA・PyCharm向けのNagiプラグインです。コンパイラは[別途インストール](https://nagi.disnana.com/docs/getting-started/)してください。

プラグインの版は`build.gradle.kts`で管理します。現在公開中の版は0.1.1です。[JetBrains Marketplace](https://plugins.jetbrains.com/plugin/34891-nagi)から対応IDEへ直接インストールするか、[GitHub Releases](https://github.com/disnana/Nagi/releases)からIDE別ZIPを取得できます。

## できること

- `.nagi`（High）と`.low`（Low）の色分け。
- 行コメント、括弧の対応・補完、ブロックの折りたたみ。
- 改行時のインデント補助。Highは`def main():`や`case`、Lowは`{`、両方で括弧内の改行に対応します。
- **Nagi: Check**で型検査、**Nagi: Run**で実行。近くに`nagi.toml`があればプロジェクトとして扱います。
- `def main()`・`async def main()`・Lowの`fn main()`の左にある▶からも実行できます。
- コンパイラ出力のファイル位置をクリックしてソースへ移動。

全体の自動整形、型に基づく補完、定義への移動、自動型検査は未対応です。型検査は手動でNagiコンパイラを呼び出します。

## インストール

`jetbrains-v0.1.1`には、IDEA用`nagi-jetbrains-IC-0.1.1.zip`とPyCharm用`nagi-jetbrains-PC-0.1.1.zip`があり、それぞれ`.sha256`も添付されています。

GitHub ReleasesのZIPを使う場合は、IDEの **Settings → Plugins → ⚙ → Install Plugin from Disk** で選び、IDEを再起動します。PRの動作確認用ZIPは、IDEA/PyCharmのstable/EAP検証がすべて成功した **Actions → Nagi checks** の`release-jetbrains` artifactから取得できます。自己ビルドした場合は`build/distributions/`にあるZIPを使います。

**Settings → Languages & Frameworks → Nagi** でコンパイラのパスを設定できます。空欄なら`PATH`の`nagic`を使います。相対パスはIDEプロジェクトのルートから解決します。

Nagiファイルを開き、右クリックまたはToolsメニューから **Nagi: Check** / **Nagi: Run** を選びます。実行前に開いているファイルを保存します。信頼していないプロジェクトではコンパイラを起動しません。

`main`の左の▶は同じ **Nagi: Run** を起動します。IDE上部で選ばれているPythonなどの実行設定は使いません。近くに`nagi.toml`がある場合は、そのプロジェクトのentryを実行します。

出力はRunウィンドウに表示します。型検査の制限時間は既定で30秒、設定で1〜300秒に変更できます。サーバーなどのRunには時間制限を設けず、RunウィンドウのStopで終了します。生成物はIDEのsystemディレクトリ内の`nagi`に置きます。

Nagiアプリのビルド・実行にはRust/CargoとOSごとのビルド環境も必要です。

## ビルドと検証

JDK 21が必要です。

```sh
cd editors/jetbrains-nagi
./gradlew test buildPlugin
```

Windowsでは`gradlew.bat`を使います。Gradle Wrapperは9.4.0、IntelliJ Platform Gradle Pluginは2.14.0、既定のSDKはIntelliJ IDEA Community 2025.1.1です。初回はSDKと依存関係を取得します。

PyCharm用SDKでも同じコードを検証できます。

```sh
./gradlew -PplatformType=PC -PplatformVersion=2025.1.1 test buildPlugin
```

手元のIDEをSDKに使う場合は`-PlocalPlatformPath=/path/to/ide`を指定します。`runIde`は開発用の別環境でIDEを起動します。

テストには字句解析・折りたたみ・インデント・CLI引数・診断位置の検証と、IntelliJ Platformの実エディターfixtureを使ったファイル種別・改行・コメント・括弧補完・保存失敗の検証があります。実プロセスを使った起動中止の検証も含みます。`NAGI_TEST_COMPILER`に`nagic`のパスを設定すると、High・Low・プロジェクトの実型検査も確認します。JetBrains CIは同じコミットからコンパイラをビルドし、IDEAとPyCharmの双方でこの連携テストを実行します。

主対象の2025.1.1は既存の対応対象です。CIはstable IDEA向けの共通ZIP候補を先に1回だけビルドし、IDEA/PyCharmのstable（2025.1.1）とEAPの計4経路で同一候補のIDE testとPlugin Verifierを実行します。最低対象のIDEA 2024.3.7とPyCharm 2024.3.6（build 243）はPlugin Verifierのみで確認し、そこでIDE testは行いません。四経路すべてが成功した後にだけ、元の候補ZIPを配布artifactへ昇格します。公開中の0.1.1に付属する二つのZIPは以前の配布物であり、今後の共通ZIPとは別です。これらのチェックはIDE全体を操作するテストとは異なります。

通常の互換性確認は`./gradlew test buildPlugin verifyPlugin`を実行します。Verifierでは要求された表示名に関する`TemplateWordInPluginName` lintだけをmuteし、API互換性・deprecated・experimentalの警告はmuteしません。最低対象も確認するには、IDEAでは`-PminimumPlatformVersion=2024.3.7`、PyCharmでは`-PplatformType=PC -PminimumPlatformVersion=2024.3.6`を追加します。最新EAPの検査では`-PplatformVersion=LATEST-EAP-SNAPSHOT`を指定します。`-PlocalPlatformPath`を指定した場合は、そのローカルSDKだけを検証します。

公式資料: [Plugin SDK](https://plugins.jetbrains.com/docs/intellij/developing-plugins.html)、[Gradle Plugin](https://plugins.jetbrains.com/docs/intellij/tools-intellij-platform-gradle-plugin.html)。ライセンスは[MIT](LICENSE)です。
