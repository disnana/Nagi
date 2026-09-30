# Nagi Language for VS Code

Nagi High (`.nagi`) とLow (`.low`)の開発補助です。

- 構文の色付け、コメント、括弧・引用符、4空白のインデント
- class・関数・HTTP・借用・Low置換のスニペット
- ファイルを開いた時と保存時の`nagic check`、Problemsへの診断表示
- コマンドパレットの型検査、Low変換、ビルド、実行
- エディター右上の実行ボタン
- `nagi.toml`の入口・Rust依存・Low設定を使ったプロジェクトの検査と実行
- F12 /「定義へ移動」で関数・class・import先へ移動
- ホバーで関数の引数・戻り値、async、classのフィールドを表示
- プロジェクト内の関数・class・型と、代表的な組み込み関数の補完
- 呼び出し時の引数ヒント、class生成時の名前付き引数の挿入

## インストール

リポジトリのルートで `python editors/vscode-nagi/scripts/package_vsix.py` を実行すると、`build/distribution/nagi-language-0.1.3.vsix` ができます。VS Codeの「拡張機能: VSIXからのインストール」で選択するか、次のコマンドを実行してください。

```powershell
code --install-extension build/distribution/nagi-language-0.1.3.vsix
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

## 定義へ移動（0.1.2）

ファイルを保存してから、関数の呼び出し・classの型注釈や生成箇所にカーソルを置いてF12を押します。`import "models.nagi"`の文字列では、そのファイルの先頭へ移動します。High・Low両方に対応し、プロジェクトの入口から読み込まれたファイルや手書きLowの定義を対象にします。

たとえば`test-nagi-code/result-api/server.nagi`の`read_item`から`storage.nagi`へ、`Item`から`models.nagi`へ移動できます。ソースの場所はコンパイラの`symbols`コマンドから取得します。型エラーがあるコードでも構文とimportが読み込めれば使えます。

プロジェクト内に未保存のファイルがある間は移動せず、保存するよう案内します。単独ファイルも保存が必要です。読み込みが失敗した場合はOutputの「Nagi」で確認できます。

対象は関数・class・importです。ローカル変数・引数・caseのpayload、組み込み関数、Rust実装への移動は未対応です。Lowの置換関数とHighの宣言が両方ある場合は、Highの宣言を優先します。

## ホバー・補完・引数ヒント（0.1.3）

最新版の`nagic`と拡張0.1.3を使います。関数名にマウスを置くと引数・戻り値・asyncの宣言が表示され、class名ではフィールド一覧を確認できます。`Result[Item?, Error]`や`view[str]`などの型も宣言どおりに表示します。

名前を書きかけるかCtrl+Spaceを押すと、入口からimportされた関数・class、手書きLowの関数、代表的な組み込み関数の候補が出ます。関数を選ぶと位置引数の入力欄、classを選ぶと`Item(id=..., name=...)`の名前付き引数が入り、Tabで次の欄へ進めます。型注釈・戻り値の位置ではclass・型・`Result` / `List` / `view`などを候補にします。

`(`や`,`を入力すると引数ヒントが出て、入力中の引数が選ばれます。既に`(`がある名前の補完では括弧を重複挿入しません。asyncとResultの宣言を見て、呼び出しに`await`、`try`、`match`が必要かを判断してください。

一度保存した`.nagi` / `.low`の編集中の内容と、開いているimport先の未保存の宣言をメモリ上で読みます。型エラーがあっても、構文とimportが読めれば宣言情報を使えます。編集中のファイルを自動保存したり、ビルドしたりはしません。`nagi.toml`の変更は保存してから使ってください。

書きかけの`add(1, `などで構文を読めない場合は、保存済みのプロジェクトの宣言を候補にします。その場合は「保存済み」と表示します。コメント・文字列の中には名前の補完やホバーを出しません。

この版の型表示は関数・classに宣言された型を対象にします。ローカル変数の推論型、`value.field`の候補、caseのpayload名、Rust実装の解析は今後の範囲です。F12は引き続き保存後に使います。

## 開発

Node.jsで `node --test editors/vscode-nagi/test/*.test.js` を実行できます。VS CodeのExtension Development Hostでは、`--extensionDevelopmentPath`にこのフォルダーを指定して試せます。ローカル変数の推論型表示、値のフィールド候補、rename、デバッグは未対応です。
