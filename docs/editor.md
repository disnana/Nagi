# VS Codeで型と補完を使う

[目次](README.md) · [準備と最初の実行](getting-started.md) · [拡張のインストールと設定](../editors/vscode-nagi/README.md)

Nagi拡張0.1.8と最新版の`nagic`を用意し、NagiリポジトリをVS Codeで開きます。このページでは、動くコードを使って型ホバー・フィールド補完・定義への移動を試します。HighとLowの両方で使えます。

## サンプルを開く

[examples/tutorial/editor_types.nagi](../examples/tutorial/editor_types.nagi)を開いてください。内容は次の完全なコードです。

```nagi
class Count:
    value: i64

def parse_count(text: str) -> Result[Count, Error]:
    number = try parse_i64(text)
    return ok(Count(value=number))

def show(count: Count):
    print(count.value)

def main():
    result = parse_count("42")
    match result:
        case Ok(count):
            show(count)
        case Err(problem):
            print(error_message(problem))
```

右上の実行ボタン、またはコマンドパレットの「Nagi: 実行」で動かすと`42`を表示します。実行はファイルを保存してから行います。

## 変数の型を確認する

次の名前にマウスを置いてください。型注釈を省略した変数も、コンパイラが求めた型を確認できます。

| 名前と場所 | 表示する型 | 理由 |
|---|---|---|
| `parse_count`の`text` | `text: str` | 引数の型注釈 |
| `number` | `number: i64` | `parse_i64`の成功値をtryで取り出す |
| `show`の`count` | `count: Count` | 引数の型注釈 |
| `main`の`result` | `result: Result[Count, Error]` | `parse_count`の戻り値 |
| Ok側の`count` | `count: Count` | Resultの成功値 |
| Err側の`problem` | `problem: Error` | Resultの失敗値 |

caseの名前はそのcaseの中だけで使えます。matchのあとに`count`を書いても、Ok側の型情報を表示することはありません。if・while・for・scopeの中で新しく作った変数も、ブロックを出ると使えません。型注釈のない整数は通常`i64`、小数は`f64`です。詳しくは[型と推論](types.md)を参照してください。

## フィールドを補完する

`show`の`print(count.value)`から`value`を消し、`print(count.)`にします。ドットのあとで候補が出なければCtrl+Spaceを押してください。`value: i64`を選ぶと、元の`count.value`に戻ります。`count.va`まで書いてから補完することもできます。

classを返す関数の`make().`、入れ子の`container.item.`、Copy classの配列の`items[0].`も、その式の型に対応するフィールドを候補にします。候補を選んだときに挿入するのはフィールド名だけです。

`result`の型は`Result[Count, Error]`なので、`result.`にはCountのフィールドを出しません。上の例のようにmatchで中のCountを取り出します。tryを使う関数では、`count = try parse_count("42")`と変数に受けるか、`(try parse_count("42")).value`と括弧で囲みます。awaitの結果のフィールドも`(await fetch()).field`と書きます。

## 関数を呼ぶ・定義を探す

関数名を書きかけるかCtrl+Spaceを押すと、プロジェクト内の関数・classと組み込み関数の候補が出ます。関数を選ぶと引数の入力欄が入り、Tabで次の欄へ移動します。classの生成では`Count(value=...)`のように名前付き引数が入ります。`(`や`,`の入力時には、引数ヒントで順番と型を確認できます。

`parse_count`や`Count`にカーソルを置いてF12を押すと定義へ移動します。importの文字列ではそのファイルを開きます。一度保存したファイルなら、未保存の編集や開いているimport先の変更も使います。

変数名でもF12を使えます。上のサンプルで、次の移動を試してください。

| カーソルを置く名前 | 移動先 |
|---|---|
| `parse_i64(text)`の`text` | `parse_count`の引数`text: str` |
| `Count(value=number)`の`number` | `number = try parse_i64(text)` |
| `print(count.value)`の`count` | `show`の引数`count: Count` |
| `match result`の`result` | `result = parse_count("42")` |
| `show(count)`の`count` | `case Ok(count)` |
| `error_message(problem)`の`problem` | `case Err(problem)` |

再代入した名前は最初の定義へ戻ります。forで外側と同じ名前を使うと、ループ内ではforの名前へ、ループ後では元の定義へ戻ります。if・while・scope・caseの中で新しく作った名前は、そのブロック内だけが対象です。move後の使用や初期化の型エラーがあっても、束縛先を特定できる名前には移動できます。classのフィールド名や組み込み関数へのF12は未対応です。

## 編集中の情報が出ないとき

一度保存した`.nagi`と`.low`は、未保存の編集をメモリ上で解析します。開いているimport先や手書きLowの編集も反映します。エディターの問い合わせでソースを保存したり、ビルドしたりはしません。新しいファイルと`nagi.toml`は保存してから使ってください。

| 状態 | 表示する情報・確認すること |
|---|---|
| `value.`のフィールド名だけを書きかけている | 受け手の型が分かればフィールド候補を出す |
| 別の場所に閉じていない括弧などがある | 保存済みの関数・classの宣言を「保存済み」と示す。ローカル型・フィールド候補・F12は止める |
| 未定義の型・変数、move後の値、スコープ外の名前 | 推測した型は出さない。Problemsでエラーを確認する |
| ローカル型・フィールド補完・ローカル変数へのF12が使えない | コンパイラと拡張を両方更新する |
| 補助ファイルの関数が見つからない | [nagi.toml](projects.md)のentryからimportされているか確認する |
| すべての問い合わせが動かない | ワークスペースの信頼、`nagi.compilerPath`、Outputの「Nagi」を確認する |

未保存の編集では、古いProblemsの診断を消します。診断を更新するには保存するか「Nagi: 型検査」を実行してください。ホバーや補完で型が見えていても、プログラム全体の型検査が成功したとは限りません。
