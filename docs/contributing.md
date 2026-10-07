# Nagi本体へ初めて貢献する

[Docsの目次](README.md) · [不具合を再現して動かす](first-app.md) · [リポジトリの貢献方針](../CONTRIBUTING.md)

この手順は、Nagiのcompiler、runtime、標準library、Docsへ最初の変更を送る人向けです。ここでは、compilerの既存動作を調べて小さい修正を送る流れを説明します。構文、型、ownership、失敗、非同期処理、resourceの終了方法を変える提案は、実装前にIssueで挙動と互換性を相談してください。

## 1. 作業環境とbranchを用意する

Git、stable Rust/Cargo、rustfmt、Clippy、Cのビルド環境を用意します。OSごとのRust・C toolchainは[セットアップ](getting-started.md)にあります。GitHubでNagiのforkを作ってから、`YOUR_GITHUB_NAME`を自分のGitHub名へ置き換えてcloneします。rootでupstreamの最新`main`から作業branchを作ります。

```sh
git clone https://github.com/YOUR_GITHUB_NAME/Nagi.git
cd Nagi
git remote add upstream https://github.com/disnana/Nagi.git
git fetch upstream
git switch -c test/nullable-default upstream/main
git remote -v
cargo --version
rustc --version
cargo build --locked -p nagic
```

`git remote -v`には書き込み先のfork `origin`と、本家の読み取り用`upstream`が表示されます。forkを使わず本家へpushする権限がある場合は、本家を`origin`にしても構いません。

compilerの開発buildは`target/debug/nagic`（Windowsは`target\debug\nagic.exe`）にできます。次で起動を確かめます。

```sh
./target/debug/nagic --version
```

期待する表示は`nagic 0.1.11`です。Cargoが見つからない場合はRust導入とPATHを確認して新しいterminalを開きます。C compilerやlinkerのエラーは、Nagiの型検査失敗とは分けて[OSのbuild準備](getting-started.md#アプリのビルドに必要なもの)を確認してください。

Windows PowerShellでは実行ファイルを`& .\target\debug\nagic.exe --version`で起動します。以下の`./target/debug/nagic`は以降この開発buildを表します。

## 2. 変更する層と検査を見つける

リポジトリrootをcurrent directoryにします。まず[Nagi compilerの流れ](compiler-internals.md)と[内部のpipeline map](internal/compiler-pipeline.md)を読み、公開挙動を変える可能性があれば[言語・runtimeの契約](internal/language-invariants.md)も確認します。

| 調べる場所 | 主な役割 |
|---|---|
| `compiler/src/lexer.rs`、`parser.rs` | High/Lowのtoken化と構文解析 |
| `compiler/src/check.rs`、`view_flow.rs` | 型、ownership、borrow、asyncの検査 |
| `compiler/src/emit.rs` | HighからLow、checked programからRustへの生成、CLI |
| `runtime/src/` | HTTP、SQLite、Task、Actorなどの実行時API |
| `compiler/tests/`、`tests/conformance/` | 単体・統合testとHigh/Lowの回帰corpus |
| `docs/`、`docs/en/`、`website/` | 日本語・英語Docsと静的サイト |

ここでいう[コードマップ](code-map.md)は、Nagiアプリを`nagic map`で静的に図にする機能です。Rustで書かれたリポジトリ自体を解析する図ではありません。ソースの場所は`rg`で検索し、parser/checker/codegenの役割は上記のcompiler資料でたどります。

`rg`（ripgrep）が使える場合、アプリ例にある`parse_i64`の使用箇所とcompilerの登録場所をrootから次のように探せます。

```sh
rg -n 'parse_i64' compiler/src runtime/src tests/conformance compiler/tests
```

出力には、例の呼び出し元に加えて`compiler/src/stdlib.rs`のような登録箇所が含まれます。期待するfileが見つからない場合は、まず検索語を短くし、`rg --files compiler/src runtime/src compiler/tests`で候補fileを確認します。

`rg`をまだ導入していない場合は、Gitに含まれるfileを`git grep -n 'parse_i64' -- compiler/src runtime/src tests/conformance compiler/tests`で検索できます。

## 3. 問題を小さい例で再現する

Issueの再現codeを別fileへ保存し、まずNagi compilerの検査段階を記録します。既存のtutorial例はrootから次のように検査できます。

```sh
./target/debug/nagic check examples/tutorial/first_app.nagi
```

成功時は終了コード0でNagiの診断がありません。実行結果も必要なcompiler/runtimeの変更では、stdinを与えて実行します。

```sh
printf '1000\n' | ./target/debug/nagic run examples/tutorial/first_app.nagi
```

Windows PowerShellでは、たとえば`"1000" | & .\target\debug\nagic.exe run examples\tutorial\first_app.nagi`を使います。実行例の期待結果は[CLIアプリの入力表](first-app.md#3-境界値を試して間違いを直す)にあります。

失敗段階を区別してください。`check`はNagi parser/checker、`build`は生成Rust・Cargo/rustcも含み、`run`はさらにアプリを実行します。build環境や外部crateの失敗を、Nagiの構文・ownership不具合の証拠として扱わないでください。逆に、Nagiが受理したcodeをNagiの型・move・lifetimeの問題で生成Rustが拒否するなら、compiler bugとして扱います。

## 4. 最小の修正と回帰を用意する

担当fileの実装と、既存testが何を保証するかを先に読みます。syntaxやcheckerの変更では、期待する有効例と拒否例、High/Low、診断の元file・lineを揃えます。codegenやownershipの変更では、実際のRust build/runと観測値が必要です。runtime変更では正常終了と失敗・panic・取消・resource終了のうち、影響する境界を既存harnessに沿って確認します。

テストを重複させず、既存のsuiteを広げられるか確認します。

```sh
cargo test --locked -p nagic --test conformance
cargo test --locked -p nagic --test explicit_moves
cargo test --locked -p nagi-runtime --lib
```

最初の2つはcompiler regression corpusと明示moveのHigh/Low/Rust検査、3つ目はruntime library testを実行します。変更した範囲に合うtargetを選びます。`--test`を指定したtest binaryが存在しない場合は、`compiler/tests/`のfile名と`cargo test --locked -p nagic --test <file-name>`を合わせてください。testの段階、corpusの書き方、失敗の切り分けは[compiler test guide](internal/compiler-testing.md)を参照します。

最終確認では、変更したRustに`cargo fmt --all -- --check`と対象testを実行します。compiler/runtimeの修正ではClippyと`cargo test --locked`も必要です。Docsだけの変更ならRust全suiteは不要で、日英の対応、リンク、変更した実行例を確認します。

### 小さな例：nullableのconformance fixtureを追加する

この例では実装を変えず、`Some`と`None`を処理する回帰fixtureを追加します。repository rootで新しい`tests/conformance/nullable_default.nagi`を作ります。

```nagi
def number_or_zero(value: i64?) -> i64:
    match value:
        case Some(number):
            return number
        case None:
            return 0
```

次に`tests/conformance/corpus.json`の配列へ、次のcaseを加えます。ほかのcaseと同じく、各項目の間にcommaを置いてください。

```json
{
  "name": "nullable_default",
  "source": "nullable_default.nagi",
  "high": true,
  "expected": "run-pass",
  "oracle": "assert_eq!(number_or_zero(Some(42i64)), 42i64); assert_eq!(number_or_zero(None), 0i64);",
  "diagnostic": "",
  "line": 0
}
```

`high: true`の正例はHighと保存Lowを検査し、`oracle`は両方のnative実行結果を照合します。まずソースをcheckし、続けてこのsuite内のcorpus＋生成caseを実行します。どちらもrepository rootから実行します。

```sh
./target/debug/nagic check tests/conformance/nullable_default.nagi
cargo test --locked -p nagic --test conformance corpus_and_bounded_generated_contracts_reach_native_execution -- --exact
```

check成功時は`checked tests/conformance/nullable_default.nagi`と表示します。test成功時は指定testが`ok`となり、summaryが`1 passed`です。このtestは新caseだけでなく、登録済みcorpusと既定のbounded generated casesも実行します。失敗時は報告されたcase名・stage・元行と、fixture・`corpus.json`のsource/oracleを確認します。期待値を変えて結果を合わせる前に、契約と独立した正しい挙動を確認してください。

サイトDocsを更新したときはPython 3.12以降と`website/requirements.txt`の依存を用意し、rootから次を実行します。

```sh
python website/build.py --base-path / --out build/website-preview
```

成功すると`build/website-preview/`に日本語・英語のHTMLが生成され、リンクと見出しanchorも検査されます。`ModuleNotFoundError`なら[websiteの手元での確認](../website/README.md#手元で確認する)の仮想環境を使います。リンクが失敗したら、表示されたMarkdownのsource linkとanchorを修正して再実行します。

## 5. 差分を見直す

repository rootから、無関係な変更や生成物が混ざっていないか確認して、今回の2 fileをstageします。

```sh
git add tests/conformance/corpus.json tests/conformance/nullable_default.nagi
git diff --cached --check
git status --short
git diff --cached -- tests/conformance/corpus.json tests/conformance/nullable_default.nagi
```

期待するのは、空白エラーがなく、`corpus.json`の変更とfixture追加だけが見えることです。違うfileが出たらcommit前に外し、原因を確認します。

## 6. commitしてforkからPRを出す

差分を自分で確認した後、repository rootからcommitします。

```sh
git commit -m "Add nullable conformance case"
git status --short
git push -u origin test/nullable-default
```

commit後の`git status --short`が空なら、未commitの変更はありません。pushが成功すると、`origin`の`test/nullable-default`へbranchが作成されます。GitHubでforkの **Pull requests → New pull request** を開き、base repositoryを`disnana/Nagi`、base branchを`main`、compare repositoryを自分のfork、compare branchを`test/nullable-default`に選びます。差分を確認し、タイトル、問題、変更後の動作、実行したtestと結果、未確認の範囲を記入してPRを作成します。

PRは必ず最新`main`向けです。forkでbranchが見つからない場合はpush先のremoteとbranch名を確認します。pull requestの基準は[rootのCONTRIBUTING](../CONTRIBUTING.md)を参照してください。

### 困ったとき

| 症状 | 次に確認する場所 |
|---|---|
| `nagic`では新しい変更が見えない | PATH上の公開版でなく`./target/debug/nagic`を実行し、必要なら再build |
| `check`は通るが`build`が失敗 | 生成Rust・Cargo/rustc診断とbuild環境を分ける。[compiler internals](compiler-internals.md)も参照 |
| testが失敗する | 最初の失敗stage、expected/actual、元source lineを確認。仕様契約を緩めて通さない |
| site buildでimport error | websiteのPython venvを有効にし、`pip install -r website/requirements.txt`を実行 |
| 手を付けるべき場所が決まらない | Issueの再現から`rg`で使用箇所を探し、既存testと契約から変更範囲を決める。契約変更なら先に相談 |
