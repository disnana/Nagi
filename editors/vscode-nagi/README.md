# Nagi for VS Code

Nagiの`.nagi`（High）と`.low`（Low）を編集・検査・実行する拡張です。[English](https://nagi.disnana.com/en/docs/vscode-extension/)

このREADMEは拡張0.1.13を説明します。実行前の変更確認と標準APIの定義表示の修正を含みます。版ごとの変更は[CHANGELOG](../../CHANGELOG.md)で確認できます。

| 機能 | コンパイラ |
| --- | --- |
| 色付け、スニペット、括弧補完、インデント補助、Lowの折りたたみ | 不要 |
| キーワード・型・組み込み関数の補完、ホバー、引数ヒント | 不要 |
| プロジェクトの型ホバー・フィールド補完・定義への移動 | 必要 |
| 型検査、Low変換、ビルド、実行 | 必要 |

## インストール

[Marketplace](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang)からインストールします。

```sh
code --install-extension Disnana.nagi-lang
```

古いIDの`nagi-local.nagi-language`・`Disnana.nagi-language`がある場合は無効化するか削除してください。[GitHub Releases](https://github.com/disnana/Nagi/releases)から取得する場合、確認時点の公開ファイルは`nagi-language-0.1.13.vsix`と対応する`.sha256`です。VS Codeの**拡張機能: VSIXからのインストール**で選びます。

コンパイラは含まれません。[Nagiのインストール](https://nagi.disnana.com/docs/getting-started/)後、VS Codeを再起動してください。型検査には`nagic`、build・runにはRust/CargoとOSのビルド環境も必要です。

コンパイラは`nagi.compilerPath`の指定を優先します。空欄ならリポジトリの`target/release`・`target/debug`、次にPATHから探します。`spawn nagic.exe ENOENT`は起動先が見つからないという意味です。設定とOutputの**Nagi**を確認してください。起動失敗はソースの型エラーとして表示しません。

## 入力時のインデント

`def main():`・`async def`・`if`・`match`・`case`などの後でEnterを押すと、1段字下げします。`else:`・`case ...:`の最後のコロンを入力すると、対応するブロックの位置に揃えます。

括弧内の改行と閉じ括弧の位置も補助します。Lowでは波括弧も対象です。コメント・文字列内の記号は対象外です。既定は4スペースで、幅と空白・タブの選択はエディターの設定に従います。

入力中の位置調整を無効にする設定：

```json
{
  "[nagi]": { "editor.formatOnType": false },
  "[nagi-low]": { "editor.formatOnType": false }
}
```

コード全体の自動整形には対応していません。

## 手書きLow

Lowは波括弧で書く別構文です。`.low`では`fn`・`record`表記で補完とホバーを表示し、波括弧のブロックを折りたためます。

[注文見積もりCLI](https://github.com/disnana/Nagi/tree/main/test-nagi-code/low-examples/order-quote)でLowのimport・record・enumを試せます。Highの関数をLowへ差し替える場合は[HighとLow](https://nagi.disnana.com/docs/low-language/)を参照してください。

## 設定

コンパイラを自動探索する設定例です。

```json
{
  "nagi.compilerPath": "",
  "nagi.checkOnSave": true,
  "nagi.checkTimeoutMs": 15000
}
```

`compilerPath`に相対パスを指定すると、ワークスペースのフォルダーを基準に解決します。`nagi.nativeFiles`・`nagi.rustFile`・`nagi.rustDependencies`でもCLI引数を追加できます。アプリごとの設定には次の`nagi.toml`を使ってください。

自動検査はファイルを開いた時と保存時です。編集すると古い診断を消します。キー入力ごとの再検査は行いません。**Nagi: 型検査**は、一度保存したファイルの未保存の内容を読み、保存やビルドをせず検査します。新規ファイルと`nagi.toml`は先に保存してください。

未信頼のワークスペースではコンパイラを起動しません。色付け・インデント補助・組み込みの入力補助は利用できます。

## プロジェクト

[nagi.toml](https://nagi.disnana.com/docs/projects/)に入口・Rust依存・手書きLowを設定します。開いたファイルから親へ最も近い設定を探し、補助ファイルからでも`entry`を検査・実行します。設定がなければ、開いたファイルだけを処理します。

右上の▶、またはコマンドパレットの**Nagi: 実行**で実行できます。lower・build・runは、プロジェクトと読み込まれるimport先の未保存ファイルを先に保存します。mainの実装では、保存の失敗や実行準備中のソース・設定変更を検出すると中止します。

診断はProblems、コンパイラの出力はOutputの**Nagi**、ビルド・実行の出力はターミナルに表示します。Rustや依存ライブラリの診断もターミナルで確認してください。`check`の成功はRust側の検査の成功を意味しません。

## 定義へ移動

F12で関数・class・enum・enumの種類・ローカル変数・import先へ移動します。moduleの修飾名やfromの別名も対象です。再代入した変数は最初の束縛へ移動します。

一度保存したHigh・Lowと、開いているimport先の未保存の変更を使います。構文やimportを読み込めず保存済み情報へ切り替わった場合、古い位置へは移動しません。

標準APIの名前では、コンパイラが提供する読み取り専用リファレンスを開きます。classのフィールド名、通常の組み込み関数、Rust実装への移動は未対応です。

## ホバー・補完・引数ヒント

| 入力・操作 | 表示 |
| --- | --- |
| 変数にホバー | コンパイラが確認できた型 |
| `item.` | classのフィールドと型 |
| `orders.` | importしたmodule自身の定義 |
| `AuthError.` | enumの種類 |
| `http.Status.`・`http.Method.` | 標準HTTPの定数 |
| 関数名・class名の補完 | 引数の入力欄。classは名前付きフィールド |
| `(`・`,` | 呼び出しの引数ヒント |

`std.http.server`・`std.actor`の型・関数・定数も扱えます。同名の型はimport先で区別します。nullableは`Some`・`None`、Resultは`Ok`・`Err`で中の値を取り出してください。

型が不明な名前やmove後の値のフィールドは補完しません。書きかけの構文を解析できない場合は、保存済みの宣言を「保存済み」と表示し、ローカル型・フィールド補完・F12は止めます。コメントと文字列内にも補完を出しません。[操作例](https://nagi.disnana.com/docs/editor/)を参照してください。

参照検索、rename、デバッグ、Rust実装の解析は未対応です。

## 開発

リポジトリのルートでコンパイラをビルドし、次を実行します。

```sh
node --test editors/vscode-nagi/test/*.test.js
python editors/vscode-nagi/scripts/package_vsix.py
```

Nodeテストは入力補助のロジックと`nagic symbols`連携を確認します。VS Code内の動作は別途Extension Development Hostで検証します。`--extensionDevelopmentPath`にこのフォルダー、`--extensionTestsPath`に`test/host.js`、ワークスペースにリポジトリのルートを指定します。

インデントだけなら`test/indentation-host.js`、コンパイラなしの入力補助なら`test/static-assistance-host.js`を使います。通常のCIでNodeテストが通ったことだけでは、実際のVS Codeでの入力・表示を検証したことにはなりません。
