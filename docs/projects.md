# nagi.tomlでアプリの設定をまとめる

複数ファイルのアプリでは、入口ファイルやRust連携の引数を`nagi.toml`に保存できます。CLIとVS Codeは同じ設定を使います。

## 最小のプロジェクト

新しいフォルダーに、次の2ファイルを置きます。

```text
my-app/
  nagi.toml
  main.nagi
```

`nagi.toml`：

```toml
entry = "main.nagi"
```

`main.nagi`（完全なコード）：

```nagi
def main():
    print("Hello, Nagi project!")
```

`my-app`で実行します。`nagic`にPATHを通していない場合は、ビルド済みコンパイラの絶対パスを使ってください。

```powershell
nagic check
nagic run
```

`check`は型検査、`run`はビルドして実行します。`lower`と`build`も同じ設定を使います。ソースの指定を省くと、現在のフォルダーから親へさかのぼり、最も近い`nagi.toml`を選びます。`my-app/src`などの子フォルダーからも使えます。

IDE向けの`nagic symbols --project my-app`は、読み込んだソースの関数・class・ローカル変数の参照位置と定義先をJSONで標準出力へ返します。ビルドや生成ファイルの書き込みは行いません。構文・importが読み込めれば、型エラーがあっても定義位置を取得できます。

JSONには関数の引数・戻り値・async、classのフィールドも含みます。型検査で確認できた変数の型は`locals`、式の型・範囲・その型のフィールドは`expressions`に入ります。位置は元ファイルの1始まりの行・UTF-16列です。確定しない型は出力しません。これは編集補助の情報で、`check`の成功を示すものではありません。

`symbols --editor-input`では標準入力の`{"files":[{"file":"main.nagi","text":"..."}]}`で既存のNagi/Lowファイルの編集中の内容を指定できます。相対パスはターミナルの作業フォルダー基準です。ディスクへ書き込まずに読み込み時のソースを置き換えます。このオプションはsymbols専用で、設定ファイルは保存済みのものを使います。

別の場所から選ぶ場合：

```powershell
nagic run --project my-app
nagic check --project my-app/nagi.toml
```

実行したアプリの作業フォルダーは`nagi.toml`のある場所です。たとえば相対パス`data.sqlite`はそこに作られ、呼び出したターミナルの場所によって変わりません。exeを直接実行するときは、そのプロセスの作業フォルダーが使われます。

## Rustと手書きLowを追加する

```toml
entry = "src/main.nagi"
native = ["native/math.low"]

[rust]
file = "native/bridge.rs"

[rust.dependencies]
serde_json = "1.0"
```

| 設定 | 内容 |
|---|---|
| `entry` | 必須。実行・検査の入口となる`.nagi`または`.low` |
| `native` | 任意。統合する手書きLowファイルの配列 |
| `rust.file` | 任意。`native` moduleとして組み込むRustファイル1つ |
| `rust.dependencies` | 任意。依存名とCargoのversion文字列またはtableの対応 |

ファイルの相対パスはすべて`nagi.toml`基準です。`entry`以外は省略できます。存在しない設定名、型の違い、空のパス、依存の重複はエラーになります。従来のversion文字列はそのまま使えます。Rustの型や実装との一致は`build`または`run`で検査します。

### ローカルcrateとfeatureを指定する

依存ごとにtableも使えます。次は[ローカルRustライブラリのサンプル](../test-nagi-code/rust-library/README.md)の設定です。

```toml
entry = "library-pricing.nagi"

[rust]
file = "native.rs"

[rust.dependencies]
pricing = { version = "0.1", path = "engine", package = "nagi-pricing-engine", features = ["volume-discount"], default-features = false }
```

| tableの項目 | 内容 |
|---|---|
| `version` | Cargoのversion指定。`path`と併記した場合もCargoがversionの一致を検査する |
| `path` | ローカルcrateのディレクトリ。`nagi.toml`の場所を基準に解決する |
| `package` | 実際のCargo package名。上の例ではRustから`pricing::`で参照する |
| `features` | 有効にするfeature名の配列。空の配列も指定できる |
| `default-features` | その依存宣言で既定featureを有効にするか。省略時はCargoの既定値`true` |

`version`か`path`の少なくとも一方が必要です。`git`・`registry`・`workspace`・`target`別依存・`dev`/`build`依存・`optional`は未対応です。

`path`は生成先やターミナルの場所にかかわらず、設定ファイル基準です。`check`・`lower`・`symbols`はCargoを呼ばず、依存を取得せず、依存crateの存在確認も行いません。Rust crateをまだ用意していなくてもNagi側を検査できます。実際のpath、crateのAPI、依存解決は`build`/`run`でCargoが検査します。

Cargoは同じpackageへの依存経路でfeatureを統合します。`default-features = false`を指定しても、別の経路が求める既定featureまで無効にはなりません。

再ビルドでは生成先の既存`Cargo.lock`を保持します。lockは解決したversionを記録しますが、ローカルcrateのソースは固定しません。固定した解決でビルドする場合は、サンプルのディレクトリで`cargo build --release --locked --manifest-path build/library-pricing/Cargo.toml`を使えます。`nagic build --locked`は未対応です。

動く例は [Rust連携サンプル](../test-nagi-code/rust-bridge/nagi.toml)です。リポジトリのルートから：

```powershell
.\target\release\nagic.exe run --project test-nagi-code/rust-bridge
```

CRC32、JSON整形、asyncのRust関数を呼びます。[タスク管理サイト](../test-nagi-code/web-demo/README.md)も入口を設定済みです。

## コマンド引数との優先順位

`nagic run main.nagi`のようにソースだけを明示した場合は、従来どおりそのファイルを単独で処理します。近くに`nagi.toml`があっても自動では読みません。

| 指定 | 動作 |
|---|---|
| `--project DIR` / `--project FILE` | その設定を使う |
| `SOURCE --project DIR` | 設定を読み、入口だけSOURCEに変更。SOURCEはターミナルの作業フォルダー基準 |
| `--rust FILE` | 設定の`rust.file`を上書き |
| `--rust-dep NAME=VERSION` | 同じ名前の依存全体をversion文字列に置換。別の名前なら追加 |
| `--native FILE.low` | 設定の`native`に追加 |
| `--out DIR` | 生成Low・Rust・Cargo.tomlの出力先を変更 |
| `--no-project` | 自動探索を使わない。SOURCEの指定が必要 |

`--rust-dep`で同名のtableを置換すると、元の`path`・`package`・`features`・`default-features`は残りません。CLIではversion文字列だけを指定できます。

引数内の相対パスはターミナルの作業フォルダー基準です。SOURCE・`--project`・`--rust`・`--out`・同名の`--rust-dep`の重複はエラーです。`--project`と`--no-project`は同時に使えません。

プロジェクトの生成コードは`build/<入口のファイル名>/`、exeは`build/native-target/release/`に出力します。たとえば`main.nagi`なら`nagi-main.exe`（Linuxでは`nagi-main`）です。プロジェクトごとにビルド先を分けるため、別のアプリも`main.nagi`という名前を使えます。

`NAGI_NATIVE_TARGET_DIR`を指定すると、依存ライブラリのビルドを複数のアプリで共有できます。この場合だけ、実行ファイル名にソースと生成先を区別する識別子を付けます（例：`nagi-main-0123456789abcdef.exe`）。実際のパスは`build`／`run`が標準エラーへ出す`native:`行で確認できます。同じ生成先へ同時にコンパイルすることは避けてください。

## VS Codeで使う

[Nagi拡張](../editors/vscode-nagi/README.md)の0.1.1以降では、開いている`.nagi` / `.low`から親へ最も近い`nagi.toml`を探します。補助ファイルを開いた状態でも、型検査・Low変換・ビルド・実行は`entry`から行います。import先のエラーは元のファイルに表示します。

`nagi.toml`を保存すると開いているNagiファイルを再検査します。プロジェクト内に未保存のファイルがある間は自動検査を待ち、手動コマンドではそのプロジェクトの編集中ファイルを保存してから処理します。入口からimportされていないNagiファイルは、そのプロジェクトの検査対象に入りません。

コンパイラの場所だけはVS Codeの`nagi.compilerPath`かPATHで指定します。従来の`nagi.rustFile`・`nagi.rustDependencies`・`nagi.nativeFiles`もコマンド引数として使え、上の優先順位に従います。`nagi.rustDependencies`の値はversion文字列だけです。依存tableは`nagi.toml`に書きます。アプリごとの設定には`nagi.toml`を使うと、VS Codeとターミナルで同じ条件を再現できます。

拡張0.1.5以降と最新版の`nagic`では、F12でプロジェクト内の関数・class・import先・ローカル変数の定義へ移動できます。引数・for・caseの名前も対象です。入口から読み込まれるHigh・Lowを扱い、開いているソースの未保存の変更もメモリ上で読みます。構文やimportを読めない場合は、保存済みの古い位置へ移動しません。新規ファイルと`nagi.toml`の変更は保存してから使います。

宣言やローカル変数の型のホバー、関数・class・型とフィールドの補完、引数ヒントも使えます。これらは一度保存したファイルの未保存の編集も読みます。import先や手書きLowの未保存の内容も対象です。書きかけで解析できない場合は「保存済み」と示して宣言の候補を出し、ローカル型とフィールド候補は出しません。[操作例](editor.md)と[拡張の設定](../editors/vscode-nagi/README.md)を参照してください。
