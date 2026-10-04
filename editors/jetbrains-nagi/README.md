# Nagi for JetBrains

[English](README.en.md)

IntelliJ IDEA・PyCharm向けのNagiプラグインです。コンパイラは別途インストールしてください。

## できること

- `.nagi`（High）と`.low`（Low）の色分け。
- 行コメント、括弧の対応・補完、ブロックの折りたたみ。
- 改行時のインデント補助。Highは`def main():`や`case`、Lowは`{`、両方で括弧内の改行に対応します。
- **Nagi: Check**で型検査、**Nagi: Run**で実行。近くに`nagi.toml`があればプロジェクトとして扱います。
- コンパイラ出力のファイル位置をクリックしてソースへ移動。

全体の自動整形、型に基づく補完、定義への移動、入力中の自動検査は今後の対応です。型検査はNagiコンパイラが行います。

## インストール

GitHubの **Actions → Nagi checks**（PR）または **Nagi JetBrains plugin**（main）で成功した実行を開き、Artifactsの`nagi-jetbrains-IC`（IDEA）または`nagi-jetbrains-PC`（PyCharm）をダウンロードします。展開するとプラグインのZIPが入っています。

その`nagi-jetbrains-*.zip`をIDEの **Settings → Plugins → ⚙ → Install Plugin from Disk** で選び、IDEを再起動します。自己ビルドした場合は`build/distributions/`にあるZIPを使います。Marketplaceへの公開はまだ行っていません。

**Settings → Languages & Frameworks → Nagi** でコンパイラのパスを設定できます。空欄なら`PATH`の`nagic`を使います。相対パスはIDEプロジェクトのルートから解決します。

Nagiファイルを開き、右クリックまたはToolsメニューから **Nagi: Check** / **Nagi: Run** を選びます。実行前に開いているファイルを保存します。信頼していないプロジェクトではコンパイラを起動しません。

出力はRunウィンドウに表示します。型検査の制限時間は既定で30秒、設定で1〜300秒に変更できます。サーバーなどのRunには時間制限を設けず、RunウィンドウのStopで終了します。生成物はIDEのsystemディレクトリ内の`nagi`に置きます。

Nagiアプリのビルド・実行にはRust/CargoとOSごとのビルド環境も必要です。

## ビルドと検証

JDK 21が必要です。

```sh
cd editors/jetbrains-nagi
./gradlew test buildPlugin
```

Windowsでは`gradlew.bat`を使います。Gradle Wrapperは8.13、IntelliJ Platform Gradle Pluginは2.3.0、既定のSDKはIntelliJ IDEA Community 2024.3.7です。初回はSDKと依存関係を取得します。

PyCharm用SDKでも同じコードを検証できます。

```sh
./gradlew -PplatformType=PC -PplatformVersion=2024.3.6 test buildPlugin
```

手元のIDEをSDKに使う場合は`-PlocalPlatformPath=/path/to/ide`を指定します。`runIde`は開発用の別環境でIDEを起動します。

テストには字句解析・折りたたみ・インデント・CLI引数・診断位置の検証と、IntelliJ Platformの実エディターfixtureを使ったファイル種別・改行・コメント・括弧補完・保存失敗の検証があります。実プロセスを使った起動中止の検証も含みます。`NAGI_TEST_COMPILER`にインストール済み`nagic`のパスを設定すると、High・Low・プロジェクトの実型検査も確認します。未設定なら、この追加smokeだけをスキップします。

IntelliJ IDEA Community 2024.3.7のSDKでエディターfixtureと配布ZIPの生成を確認しています。IDE全体の画面操作、Plugin Verifierによる互換性確認、PyCharmの動作確認は別の検証です。CIではIDEA・PyCharmの完全なSDKを使ってテスト・ZIP生成・互換性確認を行います。

CIと同じ検証を行うには`./gradlew test buildPlugin verifyPlugin`を実行します。`verifyPlugin`は対象IDEのAPIとの互換性を確認します。

公式資料: [Plugin SDK](https://plugins.jetbrains.com/docs/intellij/developing-plugins.html)、[Gradle Plugin](https://plugins.jetbrains.com/docs/intellij/tools-intellij-platform-gradle-plugin.html)。ライセンスは[MIT](LICENSE)です。
