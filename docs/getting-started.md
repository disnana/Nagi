# 準備と最初の実行

[目次](README.md) · 次：[コードを書きながら学ぶ](language-guide.md)

Nagiの公開版をインストールし、High（`.nagi`）でHello Worldを動かします。

## 1. コンパイラを用意する

ビルド済みのNagiをインストールします。自分のアプリを`nagic build`や`nagic run`でビルドするには、[Rust / Cargo](https://www.rust-lang.org/tools/install)とCのビルド環境も必要です。

### インストーラーを使う

自分のOSのコマンドを実行してください。最新の公開版を取得し、SHA-256を確認してインストールします。管理者権限は不要です。

#### Windows（PowerShell）

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.ps1')))
```

#### Linux・macOS（bash）

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.sh | bash) && export PATH="$HOME/.local/bin:$PATH"
```

インストール後、同じターミナルで確認できます。VS Codeは再起動してください。

```text
nagic --version
nagic --help
```

2026-10-07に公開したcompilerでは`nagic 0.1.11`と表示されます。`nagic -V`と`nagic version`でも確認できます。実際にインストールした版は`nagic --version`で確かめてください。版とヘルプの表示にはRustやプロジェクト設定は必要ありません。

### エディター拡張をインストールする

VS Code拡張は[Marketplace](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang)から、または[GitHub Releases](https://github.com/disnana/Nagi/releases)の`nagi-language-0.1.13.vsix`と対応する`.sha256`から入手できます。VSIXはVS Codeの**拡張機能: VSIXからのインストール**で選びます。

IntelliJ IDEA・PyCharm向けの**Nagi for JetBrains**の正式公開先は[JetBrains MarketplaceのNagiページ](https://plugins.jetbrains.com/plugin/34891-nagi)で、現在審査中です。審査中の現時点では[GitHub Releases](https://github.com/disnana/Nagi/releases)からZIPを取得します。現在の公開版0.1.1には、IDEA用`nagi-jetbrains-IC-0.1.1.zip`とPyCharm用`nagi-jetbrains-PC-0.1.1.zip`があります。IDEで**Settings → Plugins → ⚙ → Install Plugin from Disk**を選び、ZIPを指定してIDEを再起動してください。どちらの拡張にもコンパイラは含まれません。

### アプリのビルドに必要なもの

Rust / Cargoに加えて、次のビルド環境を使います。導入済みならそのまま使えます。

- **Windows**：RustのMSVC toolchain、Visual Studio Build ToolsのC++環境。
- **Linux・WSL2**：Cコンパイラ。
- **macOS**：`xcode-select --install`でCommand Line Toolsを導入。

ビルド環境とVS Code拡張はインストーラーに含まれません。同梱SQLiteのCコードもアプリのビルド時にコンパイルします。macOS版はmacOS 15のCIで検証します。

## 2. 自分で1ファイル書く

作業用のフォルダーに`hello.nagi`を作り、次のコードを保存してください。

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

`count = 3`の整数は`i64`型です。コードは`.nagi`ファイルへ保存します。

ファイル名に空白がある場合は、`nagic run "hello world.nagi"`のようにパスを引用符で囲んでください。

## 3. 検査とビルドを使い分ける

| コマンド | 何をするか | 使う場面 |
|---|---|---|
| `check hello.nagi` | Nagiの構文・型・所有権を検査 | 保存したコードの間違いを調べる |
| `lower hello.nagi` | 検査してLowを出力 | 変換後のコードを読む |
| `build hello.nagi` | Rust側の検査を含め、実行ファイルを生成 | 実行せずにビルドする |
| `run hello.nagi` | ビルドして実行 | 書いたプログラムを試す |

`run`の標準出力はアプリの出力です。コンパイラの進捗と診断は標準エラーに出るため、CLIのJSON出力もそのままパイプで渡せます。

`check`と`lower`は現在どちらもHighの生成Lowを保存します。`check`が成功しても、Rust側の型・借用などの検査で`build`が失敗する場合があります。

ビルド時のエラーには、対応を特定できる場合は元のNagi・Lowのファイル名と行を表示します。Nagi 0.1.11では、生成Rustの詳細を`--rust-diagnostics`で表示できます。手書きRustや元の位置を特定できないエラーは、通常表示にもRustの診断を残します。

```powershell
nagic check hello.nagi
nagic build hello.nagi
```

`build`が表示した`native:`行のパスをPowerShellの`&`で実行できます。単に試す場合は`nagic run hello.nagi`でビルドと実行をまとめられます。

標準の出力先は次のとおりです。

| ファイル | 内容 |
|---|---|
| `build/hello/generated.low` | Highから変換したLow |
| `build/hello/src/main.rs` | 生成したRust（build / run時） |
| `build/hello/Cargo.toml` | 生成したRustプロジェクト（build / run時） |
| `build/hello/.nagi/` | 0.1.11では、成功したbuild世代ごとの実行ファイル・生成物 |
| `native-target/` | 共有するビルド用キャッシュ |

`build/<ソースのファイル名から拡張子を除いた名前>/`に出力します。別の場所に生成する場合は`--out build/my-hello`を付けます。`NAGI_NATIVE_TARGET_DIR`はビルド用キャッシュの場所を変更します。再ビルド後の実行ファイルは別のパスになるため、`native:`で確認します。[ビルド世代と共有キャッシュ](projects.md)も参照してください。

コンパイルには配布された`runtime/`も必要です。`nagic.exe`だけを別の場所にコピーした場合は、環境変数`NAGI_ROOT`に`runtime/`のある展開フォルダーを指定してください。生成したアプリexeの配布例は[タスク管理デモ](../test-nagi-code/web-demo/README.md)にあります。

アプリが複数ファイルになったら、[nagi.tomlとプロジェクト](projects.md)で入口やRust依存をまとめられます。設定のあるフォルダーで`nagic run`と実行でき、VS Codeも同じ入口を使います。

## 4. VS Codeで書く

[Nagi拡張のインストール手順](../editors/vscode-nagi/README.md)に従ってVSIXをインストールし、作業用のフォルダーを開きます。

`.nagi`を保存すると型検査が走り、エラーがProblemsに表示されます。右上の実行ボタン、またはコマンドパレットのNagiコマンドから実行・ビルドできます。変数名のホバーで型を確認でき、`value.`を入力するとclassのフィールドが候補に出ます。F12では関数・class・import先・ローカル変数の定義へ移動できます。一度保存したファイルの未保存の編集も対象です。[VS Codeで型と補完を使う](editor.md)で試してください。

## 更新と他の導入方法

### 更新する

上のインストーラーは実行時点の最新公式Releaseを確認し、ダウンロード・検証してからコマンドを切り替えます。mainのbranch buildやVSIXはReleaseとして扱いません。既に最新版なら、同じ版を増やしません。

Windowsの保存先は`%LOCALAPPDATA%\Nagi\versions`で、PATHに追加する入口はその下の`current`です。Linux/macOSの保存先は`~/.local/share/nagi`、コマンドは`~/.local/bin/nagic`です。bash/zshの設定にもPATHを追記します。更新時も入口は変わりません。

更新前にNagiのビルドを止めてください。切り替え後の起動確認に成功したら、旧版を配布時のアーカイブと照合して削除します。使用する1版だけ残し、ロールバック用の旧版は常設しません。追加・変更されたファイルは保護します。配布物の照合不能やWindowsで使用中のファイルなどで削除できない旧版も残し、フォルダーを表示します。更新に失敗した場合は、元のコマンドとPATHを維持します。

以前の`nagi-v0.1.6/scripts/install.ps1`などのURLは0.1.6固定です。更新にはこのページの`main/scripts/install.ps1`を使ってください。Windowsの旧インストーラーが登録した版ごとのPATHも、固定の`current`へ整理します。VS Codeの`nagi.compilerPath`に旧版の絶対パスを指定している場合は、空欄に戻して自動探索するか、新しい実行ファイルの絶対パスを指定し、VS Codeを再起動してください。

版を指定する場合は次のコマンドを使います。以下は公開済みNagi 0.1.11を指定する例です。過去の版へ戻す場合も、使う1版だけ残す方針は同じです。

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.ps1'))) -Version 0.1.11
```

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.sh | bash -s -- --version 0.1.11) && export PATH="$HOME/.local/bin:$PATH"
```

保存先を変更していた場合は、再実行時も同じ`-InstallDir`（PowerShell）または`--prefix`と`--bin-dir`（bash）を指定します。`-NoPath`／`--no-path`はPATHの永続設定を変更しません。その場合は固定の入口を自分でPATHへ登録してください。別の場所へ手動展開した配布物は自動削除の対象外です。

### アンインストールする

インストーラーで導入したNagi本体、コマンドの入口、インストーラーが追加したPATH設定を削除します。Windowsでは次を実行します。

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/uninstall.ps1')))
```

Linux・macOSでは次を実行します。

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/uninstall.sh | bash)
```

削除予定だけ確認するには、PowerShellのコマンド末尾に`-WhatIf`を付けるか、bashのコマンドの`bash`を`bash -s -- --dry-run`に変更します。保存先を変更した場合は、インストール時と同じ`-InstallDir`または`--prefix`・`--bin-dir`を指定してください。独自のシェル設定ファイルを指定していた場合は、同じ`--profile`も渡します。`-NoPath`／`--no-path`はPATHの永続設定の削除を省きます。

配布物は公開アーカイブと照合してから削除します。追加・変更のある配布物や照合できない配布物はフォルダーごと残し、場所を表示します。自分のプロジェクト、Rust/Cargo、Cのビルド環境、VS Code拡張は残ります。完了後はターミナルとVS Codeを再起動してください。

### 自分で展開する

[GitHub Releases](https://github.com/disnana/Nagi/releases)から、使うOSのファイルを取得します。以下は公開済みNagi 0.1.11のファイル名です。

| 使う環境 | ダウンロードするファイル |
|---|---|
| Windows x64 | `nagi-0.1.11-windows-x86_64.zip` |
| Linux x86_64 | `nagi-0.1.11-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `nagi-0.1.11-macos-arm64.tar.gz` |
| macOS Intel | `nagi-0.1.11-macos-x86_64.tar.gz` |

アーカイブ全体を展開し、`nagic`または`nagic.exe`と`runtime/`の位置を保ってください。**展開フォルダーそのもの**をPATHに追加すると、任意の場所で`nagic`を使えます。`NAGI_ROOT`は通常不要です。GitHubの「Source code」はコンパイラ入りの配布物ではありません。

### ソースからビルドする場合

[Git](https://git-scm.com/)でリポジトリを取得します。GitHubの「Code → Download ZIP」を使う場合も、展開先で`cargo build`を実行できます。

```bash
git clone https://github.com/disnana/Nagi.git
cd Nagi
cargo build --release --locked -p nagic
```

この場合のコンパイラは`target/release/nagic`（Windowsでは`nagic.exe`）です。`target/release`をPATHに追加するか、以降の`nagic`をその実行ファイルのパスに置き換えてください。

## 困ったとき

| 症状 | 確認すること |
|---|---|
| `Cargoが見つかりません` | `cargo --version`で確認する。Rust / Cargoが導入済みならPATHを確認し、ターミナルとVS Codeを開き直す |
| `Cargoを起動できません` | 表示されたOSのエラーを確認する。Cargoの実行権限やファイルの状態に問題がないか |
| Cコンパイラやlinkerが見つからない | WindowsのC++ビルド環境、LinuxのCコンパイラがあるか |
| `nagic.exe`が見つからない | 配布フォルダーをPATHに追加し、ターミナルを開き直したか |
| `NAGI_ROOT`の`runtime/Cargo.toml`が見つからない | `NAGI_ROOT`には`runtime/`のある展開フォルダーを指定する。通常の配布では設定を解除して自動探索を使う |
| VS Codeで`spawn nagic.exe ENOENT`やコンパイラ未検出の警告が出る | VSIXにはコンパイラを含まない。Nagi本体をインストールし、VS Codeを再起動したあと「Nagi: 型検査」を実行する。別の場所にある場合は`nagi.compilerPath`にその実行ファイルを指定する |
| タブや字下げのエラー | インデントを空白4つに統一する |
| `Build failed` / `Rust backend rejected program` | 上の診断で原因を確認する。コードのエラーならNagi・Lowのファイル名と行を調べる。依存の取得失敗やビルド環境の問題なら、その診断に従う |
| Windowsでexeを更新できない | そのアプリが実行中なら停止してから再ビルドする |
| サーバーが終了しない | `serve`はリクエストを待ち続ける。ターミナルのCtrl+Cで終了する |

次は[コードを書きながら学ぶ](language-guide.md)と[最初の小さなCLIアプリ](first-app.md)で、値・入力・失敗・境界値の確認まで試してください。
