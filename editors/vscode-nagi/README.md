# Nagi Language for VS Code

Nagi High (`.nagi`) とLow (`.low`)の開発補助です。

- 構文の色付け、コメント、括弧・引用符、4空白のインデント
- class・関数・HTTP・借用・Low置換のスニペット
- ファイルを開いた時と保存時の`nagic check`、Problemsへの診断表示
- コマンドパレットの型検査、Low変換、ビルド、実行
- エディター右上の実行ボタン
- `nagi.toml`の入口・Rust依存・Low設定を使ったプロジェクトの検査と実行

## インストール

リポジトリのルートで `python editors/vscode-nagi/scripts/package_vsix.py` を実行すると、`build/distribution/nagi-language-0.1.1.vsix` ができます。VS Codeの「拡張機能: VSIXからのインストール」で選択するか、次のコマンドを実行してください。

```powershell
code --install-extension build/distribution/nagi-language-0.1.1.vsix
```

色付けとスニペットはコンパイラなしで利用できます。型検査や実行には`nagic`が必要です。Nagiリポジトリを開き、ルートで `cargo build --release --locked -p nagic` を実行してください。リポジトリのrelease・debugビルド、次にPATHからコンパイラを探します。

## 設定

```json
{
  "nagi.compilerPath": "target/release/nagic.exe",
  "nagi.checkOnSave": true,
  "nagi.nativeFiles": [],
  "nagi.rustFile": "",
  "nagi.rustDependencies": [],
  "nagi.checkTimeoutMs": 15000
}
```

Windows以外の実行ファイルは`nagic`です。`compilerPath`・`nativeFiles`・`rustFile`の相対パスはワークスペースのフォルダーを基準にします。型検査の生成Lowは、プロジェクトまたはソース別の`build/vscode-nagi/`に出力し、通常のビルド出力と分離します。

未保存の編集中コードは自動検査しません。編集中は古い診断を消し、保存後に再検査します。手動コマンドは編集中のファイルを保存してから実行します。未信頼のワークスペースではコンパイラを実行しません。診断位置は現在のコンパイラに合わせて行単位です。

## プロジェクト

入口・Rust依存・手書きLowの設定は[nagi.toml](../../docs/projects.md)にまとめられます。開いているファイルから親へ最も近い`nagi.toml`を選び、コンパイラに`--project`として渡します。補助ファイルを開いていても、`entry`から検査・実行します。プロジェクト内の編集中ファイルがある間は自動検査を待ち、手動コマンドではそのプロジェクト内のファイルを保存します。設定の保存・作成・削除でも検査を更新します。

Rust連携サンプルは`test-nagi-code/rust-bridge/bridge.nagi`を開くだけで設定を使えます。`nagi.rustFile`などの従来の設定はコマンド引数として追加し、設定ファイルに対して上書き・追加するルールはCLIと共通です。`nagic check`はRust側の実装を検査せず、実装との型の一致はビルドで検査します。import先の型エラーはそのファイルのProblemsに表示します。

設定ファイルがなければ、従来どおり開いたファイルを単独で処理します。プロジェクト機能には、このリポジトリの最新版の`nagic`が必要です。

## 開発

Node.jsで `node --test editors/vscode-nagi/test/*.test.js` を実行できます。VS CodeのExtension Development Hostでは、`--extensionDevelopmentPath`にこのフォルダーを指定して試せます。LSPによる型に基づく補完、定義ジャンプ、rename、デバッグはこの版の範囲に含みません。
