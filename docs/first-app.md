# 最初の小さなCLIアプリ

[Docsの目次](README.md) · 前：[準備と最初の実行](getting-started.md) · 次：[コードを書きながら学ぶ](language-guide.md)

入力した金額が1,000円以内かを判定するCLIアプリを作ります。Nagiの公開compilerをインストールし、Rust/CargoとCのビルド環境を用意してください。準備がまだなら[セットアップ](getting-started.md)を先に進めます。

この例は1行の入力だけを扱い、結果を画面に表示します。ファイルやDBには保存しません。データ保存を追加するときは[SQLite](database.md)の現在のAPIと制約を確認してください。

## 1. 作業フォルダーとファイルを作る

ターミナルで作業場所を選び、次のコマンドを実行します。`nagi-budget`という新しいフォルダーを作り、その中を作業場所にします。

```sh
mkdir nagi-budget
cd nagi-budget
```

このフォルダーに`first_app.nagi`を作り、次のコードを保存してください。同じコードは[リポジトリの実行例](../examples/tutorial/first_app.nagi)にあります。

```nagi
def within_budget(amount_text: view[str], limit: i64) -> Result[bool, Error]:
    amount = try parse_i64(amount_text)
    if amount < 0:
        return error("amount must be non-negative")
    return ok(amount <= limit)

def main() -> Result[unit, Error]:
    print("Enter a whole-number amount:")
    text = try read_line()
    fits = try within_budget(view(text), 1000)
    if fits:
        print("within budget")
    else:
        print("over budget")
    return ok(print("done"))
```

`mkdir`と`cd`は成功すると何も表示しません。エディターで保存したファイルが`nagi-budget/first_app.nagi`にあることを確認してください。フォルダーが既にある場合は、`mkdir`を繰り返さずそのフォルダーへ移動します。

`within_budget`は入力文字列を整数に変換します。`view[str]`は文字列を読むための借用で、`parse_i64`の失敗は`Result[i64, Error]`として返ります。`try`は失敗をこの関数の呼び出し元へ伝えます。負の金額を拒否した後、`ok`で判定結果を返します。`main`でも`try`を使うため、`main`の戻り値は`Result`です。

初出の用語は、[viewとownership](ownership.md#借用と検査の範囲)と[Result・try・match](error-handling.md)で詳しく確認できます。`unit`は値を返さない処理の型で、ここでは`print`の結果を`ok(...)`に入れています。

## 2. 検査して動かす

同じ作業フォルダーで、compilerの版とソースを確認します。

```sh
nagic --version
nagic check first_app.nagi
```

対象compilerがNagi 0.1.11なら、版表示は`nagic 0.1.11`です。`check`が成功すると終了コードは0で、Nagiの構文・型・所有権について診断がありません。`check`はRust/Cargoのビルドを行いません。

成功時は`checked`に続いて確認したファイルのpathが表示されます。失敗した場合はsourceの行を確認し、`nagic --version`で0.1.11以降を使っていること、`first_app.nagi`を作業フォルダーに保存したことを確かめてください。

LinuxとmacOSでは、標準入力から金額を渡して実行できます。

```sh
printf '700\n' | nagic run first_app.nagi
```

PowerShellでは同じ確認を次のように実行できます。

```powershell
"700" | nagic run .\first_app.nagi
```

アプリの標準出力は次のとおりです。compilerの進捗やエラーは別の診断として表示されることがあります。

```text
Enter a whole-number amount:
within budget
done
```

`run`はRust/Cargoによるアプリのbuildと実行を行います。そのため`check`が通っても、C compilerやlinkerがない、依存を取得できない、といった理由で実行前に失敗することがあります。準備は[セットアップ](getting-started.md#アプリのビルドに必要なもの)、build診断は[検査とビルド](getting-started.md#3-検査とビルドを使い分ける)を確認してください。

## 3. 境界値を試して間違いを直す

`within_budget`の最後の比較が`amount < limit`だったとします。次の3入力を使うと、上限ちょうどのときだけ結果が違うことを確かめられます。

| 入力 | 期待する出力 | 確かめること |
|---|---|---|
| `700` | `within budget` | 上限より小さい値 |
| `1000` | `within budget` | 上限と等しい値 |
| `1001` | `over budget` | 上限より大きい値 |

`1000`で`over budget`になったら、比較を`amount <= limit`へ直します。`nagic check first_app.nagi`を再実行し、3入力の結果をもう一度確かめてください。

LinuxとmacOSでは次の3コマンドを実行します。PowerShellでは右の3行を使います。

```sh
printf '%s\n' '700' | nagic run first_app.nagi
printf '%s\n' '1000' | nagic run first_app.nagi
printf '%s\n' '1001' | nagic run first_app.nagi
```

```powershell
"700" | nagic run .\first_app.nagi
"1000" | nagic run .\first_app.nagi
"1001" | nagic run .\first_app.nagi
```

各コマンドはプロンプトの後に表の判定、`done`の順で表示します。`abc`は数値変換の失敗、`-1`はアプリが作った失敗として終了コード1になります。Nagi 0.1.11でのエラー出力は次のとおりです。

| 入力 | エラー出力 | exit code |
|---|---|---:|
| `abc` | `Invalid: invalid digit found in string` | 1 |
| `-1` | `Invalid: amount must be non-negative` | 1 |

Linux/macOSでは`printf '%s\n' 'abc'`または`printf '%s\n' '-1'`を、PowerShellでは`"abc"`または`"-1"`を同じようにpipeして確認できます。診断が表示されたら[Resultの伝播と回復](error-handling.md#失敗を呼び出し元へ返す)を確認してください。

負数はアプリが作った`Error`、`abc`は`parse_i64`が返す`Error`になります。どちらも`try`を通って`main`へ伝わります。失敗をその場で別の値に置き換える方法は[Resultとエラー処理](error-handling.md)を参照してください。

開発中のNagiには組み込みの単体テストフレームワークはありません。ここでは小さな入力表を手動で実行し、期待結果と比較します。compiler本体を変更するときの自動回帰テストは[貢献ガイド](contributing.md)と[compiler test guide](internal/compiler-testing.md)を参照してください。

## 4. 次に広げる

関数、List、class、ループ、enum、nullable、`Result`、`match`の使い分けは[言語ガイド](language-guide.md)と[言語機能索引](README.md#言語機能索引)へ。入力を複数ファイルに分けるときは[importとmodule](modules-and-rust.md)、別の値を安全に受け渡すときは[ownershipとborrow](ownership.md)、Rustライブラリを呼ぶときは[Rust連携](modules-and-rust.md#rustの関数を呼ぶ)を読みます。
