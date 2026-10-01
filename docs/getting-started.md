# 準備と最初の実行

[目次](README.md) → 準備と最初の実行 → [コードを書きながら学ぶ](language-guide.md)

このページでは、Nagiのコンパイラを用意して、1ファイルのプログラムを動かします。コマンドは**このリポジトリのルート**で実行してください。

コンパイラは、書いたコードを実行ファイルへ変換する道具です。以下のコマンドはターミナルに入力します。WindowsならPowerShell、LinuxやmacOSなら端末アプリを使います。「リポジトリのルート」は、取得したNagiのフォルダーのことです。

[GitHub Releases](https://github.com/disnana/Nagi/releases)で、使うOSに合ったNagi本体のファイルを取得して展開します。

| 使う環境 | ダウンロードするファイル |
|---|---|
| Windows x64 | `nagi-0.1.3-windows-x86_64.zip` |
| Linux x86_64 | `nagi-0.1.3-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `nagi-0.1.3-macos-arm64.tar.gz` |
| macOS Intel | `nagi-0.1.3-macos-x86_64.tar.gz` |

この配布物はコンパイラとソースを含みます。`runtime/`などのフォルダーを保ったまま使ってください。GitHubが付ける「Source code」のファイルはソースだけです。VS Code拡張は別の`nagi-language-0.1.8.vsix`を使います。

ソースからコンパイラもビルドする場合は、[Git](https://git-scm.com/)を用意して次を実行します。

```bash
git clone https://github.com/disnana/Nagi.git
cd Nagi
```

Gitを使わず、GitHubの「Code → Download ZIP」から取得することもできます。ZIPを展開したフォルダーをターミナルで開いてください。

## 1. コンパイラを用意する

必要なものは[Rust / Cargo](https://www.rust-lang.org/tools/install)とCのビルド環境です。RustはNagiのコンパイラや生成したコードをビルドするために使い、CargoはRustのビルド・依存パッケージを管理します。ランタイムに同梱されたSQLiteのCコードをビルドするため、Rustだけでは足りません。最初のビルドではCargoが依存パッケージを取得します。

WindowsではRustのMSVC toolchainとVisual Studio Build ToolsのC++ビルド環境を使います。LinuxではCコンパイラを用意してください。WSL2もLinuxの手順です。macOSでは`xcode-select --install`でCommand Line Toolsを用意します。macOS版はmacOS 15のCIでビルド・実行を検証します。

コンパイラ入りの配布物では、次の`cargo build`を省略できます。NagiのアプリをビルドするためのRust/CargoとCのビルド環境は必要です。

```powershell
# Windows / PowerShell
cargo build --release --locked -p nagic
.\target\release\nagic.exe run examples/hello.nagi
```

```bash
# Linux / WSL2 / macOS
cargo build --release --locked -p nagic
./target/release/nagic run examples/hello.nagi
```

ビルドのログのあとに、次のプログラム出力が表示されます。

```text
Hello, Nagi!
4
```

## 2. 自分で1ファイル書く

リポジトリのルートに`hello.nagi`を作り、次の完全なコードを保存してください。

```nagi
def main():
    print("こんにちは、Nagi!")
    count = 3
    print(count * 2)
```

`def main():`が入口です。関数の中は空白4つで字下げします。`print`は1つの値を改行付きで表示します。

```powershell
.\target\release\nagic.exe run hello.nagi
```

Linux / WSL2では、以降の`.\target\release\nagic.exe`を`./target/release/nagic`に置き換えてください。

```text
こんにちは、Nagi!
6
```

`count = 3`の整数は`i64`になります。Pythonとして実行するコードではありません。ファイルの拡張子は`.nagi`にしてください。

ファイル名に空白がある場合は、`nagic run "hello world.nagi"`のようにパスを引用符で囲んでください。

## 3. 検査とビルドを使い分ける

| コマンド | 何をするか | 使う場面 |
|---|---|---|
| `check hello.nagi` | Nagiの構文・型・所有権を検査 | 保存したコードの間違いを調べる |
| `lower hello.nagi` | 検査してLowを出力 | 変換後のコードを読む |
| `build hello.nagi` | Rust側の検査を含め、実行ファイルを生成 | 実行せずにビルドする |
| `run hello.nagi` | ビルドして実行 | 書いたプログラムを試す |

`check`と`lower`は現在どちらもHighの生成Lowを保存します。`check`が成功しても、Rust側の型・借用などの検査で`build`が失敗する場合があります。

ビルド時のエラーは、対応する元のNagi・Lowファイル名、文や定義の行番号、その行のコードを先に表示します。import先や`@replace`の手書きLowも対象です。続く`Rust backend details`には生成Rust側の詳しい診断を残します。手書きRustや、元の位置を特定できないエラーはRustの診断を表示します。

```powershell
.\target\release\nagic.exe check hello.nagi
.\target\release\nagic.exe build hello.nagi
.\native-target\release\nagi-hello.exe
```

標準の出力先は次のとおりです。

| ファイル | 内容 |
|---|---|
| `build/hello/generated.low` | Highから変換したLow |
| `build/hello/src/main.rs` | 生成したRust（build / run時） |
| `build/hello/Cargo.toml` | 生成したRustプロジェクト（build / run時） |
| `native-target/release/nagi-hello.exe` | Windowsの実行ファイル |
| `native-target/release/nagi-hello` | Linuxの実行ファイル |

`build/<ソースのファイル名から拡張子を除いた名前>/`に出力します。別の場所に生成する場合は`--out build/my-hello`を付けます。実行ファイルの出力先は`NAGI_NATIVE_TARGET_DIR`で変更できます。

コンパイルにはこのリポジトリの`runtime/`も必要です。`nagic.exe`だけを別の場所にコピーした場合は、環境変数`NAGI_ROOT`にNagiリポジトリの場所を指定してください。生成したアプリexeの配布例は[タスク管理デモ](../test-nagi-code/web-demo/README.md)にあります。

アプリが複数ファイルになったら、[nagi.tomlとプロジェクト](projects.md)で入口やRust依存をまとめられます。設定のあるフォルダーで`nagic run`と実行でき、VS Codeも同じ入口を使います。

## 4. VS Codeで書く

[Nagi拡張のインストール手順](../editors/vscode-nagi/README.md)に従ってVSIXをインストールし、このリポジトリのフォルダーを開きます。

`.nagi`を保存すると型検査が走り、エラーがProblemsに表示されます。右上の実行ボタン、またはコマンドパレットのNagiコマンドから実行・ビルドできます。変数名のホバーで型を確認でき、`value.`を入力するとclassのフィールドが候補に出ます。F12では関数・class・import先・ローカル変数の定義へ移動できます。一度保存したファイルの未保存の編集も対象です。[VS Codeで型と補完を使う](editor.md)で試してください。

## 困ったとき

| 症状 | 確認すること |
|---|---|
| `cargo`が見つからない | Rust / CargoがPATHにあるか。インストール後にターミナルを開き直したか |
| Cコンパイラやlinkerが見つからない | WindowsのC++ビルド環境、LinuxのCコンパイラがあるか |
| `nagic.exe`が見つからない | ルートでコンパイラのビルドを終えたか |
| VS Codeで`spawn nagic.exe ENOENT`やコンパイラ未検出の警告が出る | VSIXにはコンパイラを含まない。手順1でビルドしたあと「Nagi: 型検査」を実行する。別の場所にある場合は`nagi.compilerPath`にその実行ファイルを指定する |
| タブや字下げのエラー | インデントを空白4つに統一する |
| `Rust backend rejected program` | 直前に表示されたNagi・Lowのファイル名と行を確認する。詳しい理由は続くRustの診断にある |
| Windowsでexeを更新できない | そのアプリが実行中なら停止してから再ビルドする |
| サーバーが終了しない | `serve`はリクエストを待ち続ける。ターミナルのCtrl+Cで終了する |

次は[コードを書きながら学ぶ](language-guide.md)で、変数からAPIまで順に書いてみてください。
