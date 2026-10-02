# 準備と最初の実行

[目次](README.md) → 準備と最初の実行 → [コードを書きながら学ぶ](language-guide.md)

Nagiのコンパイラを用意して、1ファイルのプログラムを動かします。インストール後は、作業用のフォルダーでコマンドを実行できます。WindowsではPowerShell、LinuxやmacOSでは端末アプリを使います。

## 1. コンパイラを用意する

Nagiのアプリをビルドするには[Rust / Cargo](https://www.rust-lang.org/tools/install)とCのビルド環境が必要です。Cargoは依存パッケージの取得とビルドを行います。同梱SQLiteのCコードもビルドするため、Rustだけでは足りません。

WindowsではRustのMSVC toolchainとVisual Studio Build ToolsのC++環境を使います。LinuxではCコンパイラを用意してください。WSL2もLinuxの手順です。macOSでは`xcode-select --install`でCommand Line Toolsを用意します。macOS版はmacOS 15のCIで検証します。

### インストーラーを使う

Nagi 0.1.6をGitHubから取得し、SHA-256を確認して、ユーザー用の場所にインストールします。WindowsはユーザーのPATHへ追加し、Linux/macOSはbash/zshの設定へPATHの1行を追記します。管理者権限は使いません。RustやCのビルド環境、VS Code拡張は別途用意してください。

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/nagi-v0.1.6/scripts/install.ps1')))
```

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/nagi-v0.1.6/scripts/install.sh | bash) && export PATH="$HOME/.local/bin:$PATH"
```

Windowsの保存先は`%LOCALAPPDATA%\Nagi\versions`、Linux/macOSは`~/.local/share/nagi`です。Linux/macOSのコマンドは`~/.local/bin/nagic`から使えます。VS Codeを開いている場合は、インストール後に再起動してください。

```text
nagic --version
nagic --help
```

版の表示は`nagic 0.1.6`です。`nagic -V`と`nagic version`でも確認できます。版とヘルプの表示にはRustやプロジェクト設定は必要ありません。

### 自分で展開する

[GitHub Releases](https://github.com/disnana/Nagi/releases)から、使うOSのファイルを取得します。

| 使う環境 | ダウンロードするファイル |
|---|---|
| Windows x64 | `nagi-0.1.6-windows-x86_64.zip` |
| Linux x86_64 | `nagi-0.1.6-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `nagi-0.1.6-macos-arm64.tar.gz` |
| macOS Intel | `nagi-0.1.6-macos-x86_64.tar.gz` |

アーカイブ全体を展開し、`nagic`または`nagic.exe`と`runtime/`の位置を保ってください。**展開フォルダーそのもの**をPATHに追加すると、任意の場所で`nagic`を使えます。`NAGI_ROOT`は通常不要です。GitHubの「Source code」はコンパイラ入りの配布物ではありません。

### ソースからビルドする場合

[Git](https://git-scm.com/)でリポジトリを取得します。GitHubの「Code → Download ZIP」を使う場合も、展開先で`cargo build`を実行できます。

```bash
git clone https://github.com/disnana/Nagi.git
cd Nagi
cargo build --release --locked -p nagic
```

この場合のコンパイラは`target/release/nagic`（Windowsでは`nagic.exe`）です。`target/release`をPATHに追加するか、以降の`nagic`をその実行ファイルのパスに置き換えてください。

## 2. 自分で1ファイル書く

作業用のフォルダーに`hello.nagi`を作り、次の完全なコードを保存してください。

```nagi
def main():
    print("こんにちは、Nagi!")
    count = 3
    print(count * 2)
```

`def main():`が入口です。関数の中は空白4つで字下げします。`print`は1つの値を改行付きで表示します。

```powershell
nagic run hello.nagi
```

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
nagic check hello.nagi
nagic build hello.nagi
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

コンパイルには配布された`runtime/`も必要です。`nagic.exe`だけを別の場所にコピーした場合は、環境変数`NAGI_ROOT`に`runtime/`のある展開フォルダーを指定してください。生成したアプリexeの配布例は[タスク管理デモ](../test-nagi-code/web-demo/README.md)にあります。

アプリが複数ファイルになったら、[nagi.tomlとプロジェクト](projects.md)で入口やRust依存をまとめられます。設定のあるフォルダーで`nagic run`と実行でき、VS Codeも同じ入口を使います。

## 4. VS Codeで書く

[Nagi拡張のインストール手順](../editors/vscode-nagi/README.md)に従ってVSIXをインストールし、作業用のフォルダーを開きます。

`.nagi`を保存すると型検査が走り、エラーがProblemsに表示されます。右上の実行ボタン、またはコマンドパレットのNagiコマンドから実行・ビルドできます。変数名のホバーで型を確認でき、`value.`を入力するとclassのフィールドが候補に出ます。F12では関数・class・import先・ローカル変数の定義へ移動できます。一度保存したファイルの未保存の編集も対象です。[VS Codeで型と補完を使う](editor.md)で試してください。

## 困ったとき

| 症状 | 確認すること |
|---|---|
| `cargo`が見つからない | Rust / CargoがPATHにあるか。インストール後にターミナルを開き直したか |
| Cコンパイラやlinkerが見つからない | WindowsのC++ビルド環境、LinuxのCコンパイラがあるか |
| `nagic.exe`が見つからない | 配布フォルダーをPATHに追加し、ターミナルを開き直したか |
| `NAGI_ROOT`の`runtime/Cargo.toml`が見つからない | `NAGI_ROOT`には`runtime/`のある展開フォルダーを指定する。通常の配布では設定を解除して自動探索を使う |
| VS Codeで`spawn nagic.exe ENOENT`やコンパイラ未検出の警告が出る | VSIXにはコンパイラを含まない。Nagi本体をインストールし、VS Codeを再起動したあと「Nagi: 型検査」を実行する。別の場所にある場合は`nagi.compilerPath`にその実行ファイルを指定する |
| タブや字下げのエラー | インデントを空白4つに統一する |
| `Rust backend rejected program` | 直前に表示されたNagi・Lowのファイル名と行を確認する。詳しい理由は続くRustの診断にある |
| Windowsでexeを更新できない | そのアプリが実行中なら停止してから再ビルドする |
| サーバーが終了しない | `serve`はリクエストを待ち続ける。ターミナルのCtrl+Cで終了する |

次は[コードを書きながら学ぶ](language-guide.md)で、変数からAPIまで順に書いてみてください。
