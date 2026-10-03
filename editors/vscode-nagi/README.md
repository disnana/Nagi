# Nagi for VS Code

Nagi High (`.nagi`) とLow (`.low`)の開発補助です。

- 構文の色付け、コメント、括弧・引用符、4空白のインデント
- 改行時の字下げ、`else`・`case`の位置調整、複数行の括弧の位置合わせ
- class・enum・関数・HTTP・借用・Low置換のスニペット
- ファイルを開いた時・保存時の`nagic check`、Problemsへの診断表示
- コマンドパレットの型検査、Low変換、ビルド、実行
- エディター右上の実行ボタン
- `nagi.toml`の入口・Rust依存・Low設定を使ったプロジェクトの検査と実行
- F12 /「定義へ移動」で関数・class・enumとその種類・import先・ローカル変数の定義へ移動
- ホバーで関数の引数・戻り値、async、classのフィールドを表示
- 引数・ローカル変数・caseの束縛名の型をホバーで表示
- `value.`の入力時に、そのclassのフィールドを補完
- importのmodule名から定義を補完し、修飾した名前のホバー・引数ヒント・F12に対応
- プロジェクト内の関数・class・型と、代表的な組み込み関数の補完
- 呼び出し時の引数ヒント、class生成時の名前付き引数の挿入

## インストール

VS Codeの拡張機能で「Nagi for VS Code」を検索するか、[Marketplace](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang)からインストールできます。コマンドで入れる場合は次を実行します。

```bash
code --install-extension Disnana.nagi-lang
```

拡張のIDは`Disnana.nagi-lang`です。以前の`nagi-local.nagi-language`や`Disnana.nagi-language`を入れている場合は、先に無効化するかアンインストールしてください。

正式版のVSIXは[GitHub Releases](https://github.com/disnana/Nagi/releases)に掲載します。ファイル名は`nagi-language-バージョン.vsix`です。mainで拡張のバージョンを上げたとき、CI成功後に自動公開します。

リポジトリのルートで `python editors/vscode-nagi/scripts/package_vsix.py` を実行すると、`build/distribution/nagi-language-0.1.9.vsix` ができます。VS Codeの「拡張機能: VSIXからのインストール」で選択するか、次のコマンドを実行してください。

```powershell
code --install-extension build/distribution/nagi-language-0.1.9.vsix
```

拡張0.1.9では、色付け、スニペット、インデント補助、キーワード・型の補完と組み込み関数の補完・ホバー・引数ヒントをコンパイラなしで利用できます。型検査や実行には`nagic`が必要です。[準備と最初の実行](https://disnana.github.io/Nagi/docs/getting-started/)の手順でインストールしてください。拡張はリポジトリのrelease・debugビルド、次にPATHからコンパイラを探します。

VSIXにはコンパイラ本体を含めていません。`spawn nagic.exe ENOENT`などのメッセージは、コンパイラが見つからないことを示します。インストール後にVS Codeを再起動し、「Nagi: 型検査」を実行してください。別の場所にあるコンパイラを使う場合は`nagi.compilerPath`で指定します。コンパイラの起動失敗やタイムアウトは警告とNagiの出力に表示し、ソースの型エラーとして赤線を付けません。

## 入力時のインデント

以下の入力補助は拡張0.1.9で利用できます。

`def main():`や`async def`、`enum`、`if`、`match`、`case`などのブロックの後でEnterを押すと、1段字下げします。`else:`や`case ...:`の最後のコロンを入力すると、対応する`if`や`match`の位置に揃えます。

`(`・`[`の中で改行すると1段字下げし、行頭に閉じ括弧を書くと、開き括弧のある行に揃えます。Lowでは`{`・`}`も対象です。コメントや文字列の中の記号は字下げの判断に使いません。新規の未保存ファイルと、未信頼のワークスペースでも利用できます。

既定は4スペースです。字下げ幅と空白・タブの選択は、VS Code右下の設定に従います。入力中の位置調整を止める場合は、設定から「Editor: Format On Type」を無効にしてください。Nagiだけで無効にする場合は次の設定を使います。

```json
{
  "[nagi]": { "editor.formatOnType": false },
  "[nagi-low]": { "editor.formatOnType": false }
}
```

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

自動検査は、ファイルを開いたときと保存時に行います。編集中は古い診断を消し、キー入力ごとの型検査は行いません。「Nagi: 型検査」は、現在のバッファを保存やビルドをせずに検査します。lower・build・runは、実行前にプロジェクトのファイルを保存します。新規ファイルと`nagi.toml`は先に保存してください。未信頼のワークスペースではコンパイラを実行しません。診断位置は現在のコンパイラに合わせて行単位です。

## プロジェクト

入口・Rust依存・手書きLowの設定は[nagi.toml](https://disnana.github.io/Nagi/docs/projects/)にまとめられます。開いているファイルから親へ最も近い`nagi.toml`を選び、コンパイラに`--project`として渡します。補助ファイルを開いていても、`entry`から検査・実行します。型検査は未保存のソースをメモリ上で読み、lower・build・runはプロジェクトのファイルを保存します。`nagi.toml`の編集は先に保存してください。設定の保存・作成・削除でも検査を更新します。

Rust連携サンプルは`test-nagi-code/rust-bridge/bridge.nagi`を開くだけで設定を使えます。`nagi.rustFile`などの従来の設定はコマンド引数として追加し、設定ファイルに対して上書き・追加するルールはCLIと共通です。`nagic check`はRust側の実装を検査せず、実装との型の一致はビルドで検査します。import先の型エラーはそのファイルのProblemsに表示します。

設定ファイルがなければ、従来どおり開いたファイルを単独で処理します。プロジェクト機能には、このリポジトリの最新版の`nagic`が必要です。

## 定義へ移動

関数の呼び出し・classの型注釈や生成箇所、変数名にカーソルを置いてF12を押します。`import "models.nagi"`の文字列では、そのファイルの先頭へ移動します。High・Low両方に対応し、プロジェクトの入口から読み込まれたファイルや手書きLowの定義を対象にします。

`import "orders.nagi" as orders`では、`orders.total(...)`や`orders.Order`の定義名から元の宣言へ移動します。fromの別名`SavedOrder`も、元のclassへ移動します。開いているimport先の未保存の編集も反映します。

たとえば`test-nagi-code/result-api/server.nagi`の`read_item`から`storage.nagi`へ、`Item`から`models.nagi`へ移動できます。ソースの場所はコンパイラの`symbols`コマンドから取得します。型エラーがあるコードでも構文とimportが読み込めれば使えます。

拡張0.1.5以降では、引数、代入で作った変数、forの要素、caseの束縛名にも対応します。再代入した名前は最初の定義へ戻ります。forで外側と同じ名前を使った場合は、ループ内ではforの定義、ループ後では外側の定義へ戻ります。if・while・scope・case内で作った名前は、そのブロック内を対象にします。

一度保存したファイルは、未保存の編集からも移動できます。開いているimport先や手書きLowの変更もメモリ上で読み、編集後の位置へ移動します。新規ファイルと`nagi.toml`は保存してから使います。構文やimportを読み込めず保存済み情報へ切り替わった場合は、古い位置へのジャンプを止めます。問い合わせ中にソースや設定が変わった場合も、その結果を使いません。

変数の参照先は型検査とは別に解決します。move後の使用や初期化の型エラーがあっても、定義を特定できる名前には移動できます。未定義・スコープ外の名前には移動しません。classのフィールド名、組み込み関数、Rust実装への移動は未対応です。Lowの置換関数を呼び出すとHighの宣言を優先し、Lowのローカル変数はLow内の定義へ移動します。

## ホバー・補完・引数ヒント

拡張0.1.9では、新規の未保存ファイル、コンパイラなし、未信頼のワークスペースでも、キーワード・型と組み込み関数の入力補助を使えます。書きかけの構文でも組み込み関数の説明を表示します。コンパイラで解析できない場合、ファイル内の同じ名前の関数や変数やimportと衝突しうる組み込み情報は控えます。プロジェクトの宣言、ローカル変数の型、フィールド候補、F12にはコンパイラとワークスペースの信頼が必要です。

最新版の`nagic`と拡張0.1.9を使います。関数名にマウスを置くと引数・戻り値・asyncの宣言が表示され、class名ではフィールド一覧を確認できます。`Result[Item?, Error]`や`view[str]`などの型も宣言どおりに表示します。[コードを使った操作例](https://disnana.github.io/Nagi/docs/editor/)もあります。

変数名にマウスを置くと、コンパイラが確認できた型を表示します。たとえば`count = 3`は`count: i64`、classを返す関数から作った`item`は`item: Item`です。関数の引数、`for`の要素、Resultの`case Ok(value)`と`case Err(problem)`の束縛名にも対応します。宣言と使用箇所を扱い、caseやifなどのブロックを出た名前には型を表示しません。

`item.`を入力すると、`Item`のフィールドが候補に出ます。候補には`name: str`のように型も表示し、選ぶとフィールド名だけを挿入します。`item.na`からの補完では入力途中の名前を置き換えます。classを返す呼び出し、入れ子のフィールド、Copy classの配列要素も対象です。`Result[Item, Error]`や`Item?`を自動でItemとして扱うことはありません。Resultは`match`のOk側や`(try fetch()).`で中の値を取り出してください。

`orders.`では、そのmodule自身が定義した関数・class・enumを候補にします。修飾した呼び出しやfromの別名のホバー・引数ヒントも、解決した定義を使います。ローカル変数がmodule名を隠した場合は、その変数の型とスコープに従います。importした名前をmoduleのメンバーとして自動で再公開しません。

`AuthError.`や`errors.AuthError.`では、enumの種類を補完します。情報を持つ`WeakPassword(message: str)`には位置引数を挿入し、情報を持たない`InvalidCredentials`には括弧を付けません。ホバーとF12は種類の宣言を示します。enumを持つ変数に、その種類をフィールドとして補完することはありません。

`import std.http.server as http`にも対応します。`http.`では標準の型と関数、`http.Status.`・`http.Method.`では定数を補完します。`from`で付けた別名も使えます。`request.`では`path`・`body`・`is_get`などの読み取り専用プロパティを表示します。標準の型・関数・定数やプロパティのF12は、コンパイラが提供する読み取り専用のリファレンスを開きます。

nullableの`match`では`Some(value)`と`None`を補完し、`match-option`で両方の分岐を挿入できます。

`std.actor`の`Actor[M, R, E]`・`Turn[S, R, E]`や、呼び出しエラー・イベントの定数とプロパティも同じ操作で扱えます。HTTPとactorの`Options`は、それぞれのimport先を参照します。

名前を書きかけるかCtrl+Spaceを押すと、入口からimportされた関数・class・enum、手書きLowの関数、代表的な組み込み関数の候補が出ます。関数を選ぶと位置引数の入力欄、classを選ぶと`Item(id=..., name=...)`の名前付き引数が入り、Tabで次の欄へ進めます。型注釈・戻り値の位置ではclass・enum・型・`Result` / `List` / `view`などを候補にします。

`(`や`,`を入力すると引数ヒントが出て、入力中の引数が選ばれます。既に`(`がある名前の補完では括弧を重複挿入しません。asyncとResultの宣言を見て、呼び出しに`await`、`try`、`match`が必要かを判断してください。

一度保存した`.nagi` / `.low`の編集中の内容と、開いているimport先・手書きLowの未保存の内容をメモリ上で読みます。型エラーがあっても、構文とimportが読めれば宣言情報を使えます。ローカル型やフィールド候補は、型を確認できた箇所に出します。型が不明な名前、move後の値、スコープ外の名前について推測した候補は出しません。編集中のファイルを自動保存したり、ビルドしたりはしません。`nagi.toml`の変更は保存してから使ってください。

書きかけの`add(1, `などで構文を読めない場合は、保存済みのプロジェクトの宣言を候補にします。その場合は「保存済み」と表示します。この状態ではローカル変数の型やフィールド候補は出しません。コメント・文字列の中には名前の補完やホバーを出しません。

ローカル型・フィールド補完は拡張0.1.4、ローカル変数と未保存ソースのF12は0.1.5で追加しました。古い`nagic`には新しいシンボル情報がないため、コンパイラも更新してください。参照検索、rename、デバッグ、Rust実装の解析は未対応です。

## 開発

リポジトリのルートでコンパイラをビルドし、`node --test editors/vscode-nagi/test/*.test.js`を実行します。Nodeテストには文字列処理だけのテストと、実際の`nagic symbols`を使うテストがあります。

VS Code上の確認は別に行います。Extension Development Hostに`--extensionDevelopmentPath`でこのフォルダー、`--extensionTestsPath`で`test/host.js`、ワークスペースとしてリポジトリのルートを指定します。診断・F12・ホバー・補完・引数ヒント・プロジェクトの実行と、実際の入力操作を確認します。インデントだけをコンパイラなしで確認する場合は`test/indentation-host.js`、静的な補完・ホバー・引数ヒントは`test/static-assistance-host.js`を指定します。結果はそれぞれ`build/vscode-typing-result.json`と`build/vscode-static-assistance-result.json`に残します。
