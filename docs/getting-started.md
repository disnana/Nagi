# 準備と最初の実行

[目次](README.md) → 準備と最初の実行 → [コードを書きながら学ぶ](language-guide.md)

このページでは、Nagiのコンパイラを用意して、1ファイルのプログラムを動かします。コマンドは**このリポジトリのルート**で実行してください。

## 1. コンパイラをビルドする

必要なものはRust / CargoとCのビルド環境です。ランタイムに同梱されたSQLiteのCコードをビルドするため、Rustだけでは足りません。最初のビルドではCargoが依存パッケージを取得します。

WindowsではRustのMSVC toolchainとVisual Studio Build ToolsのC++ビルド環境を使います。このリポジトリではWindowsネイティブでコンパイラとデモexeのビルド・実行を確認しています。LinuxではCコンパイラを用意してください。WSL2もLinuxの手順です。

```powershell
# Windows / PowerShell
cargo build --release --locked -p nagic
.\target\release\nagic.exe run examples/hello.nagi
```

```bash
# Linux / WSL2
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

## 3. 検査とビルドを使い分ける

| コマンド | 何をするか | 使う場面 |
|---|---|---|
| `check hello.nagi` | Nagiの構文・型・所有権を検査 | 保存したコードの間違いを調べる |
| `lower hello.nagi` | 検査してLowを出力 | 変換後のコードを読む |
| `build hello.nagi` | Rust側の検査を含め、実行ファイルを生成 | 実行せずにビルドする |
| `run hello.nagi` | ビルドして実行 | 書いたプログラムを試す |

`check`と`lower`は現在どちらもHighの生成Lowを保存します。`check`が成功しても、Rust側の型・借用などの検査で`build`が失敗する場合があります。

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

## 4. VS Codeで書く

[Nagi拡張のインストール手順](../editors/vscode-nagi/README.md)に従ってVSIXをインストールし、このリポジトリのフォルダーを開きます。

`.nagi`を保存すると型検査が走り、エラーがProblemsに表示されます。右上の実行ボタン、またはコマンドパレットのNagiコマンドから実行・ビルドできます。色付けとスニペットも利用できます。型に基づく補完や定義ジャンプはまだありません。

## 困ったとき

| 症状 | 確認すること |
|---|---|
| `cargo`が見つからない | Rust / CargoがPATHにあるか。インストール後にターミナルを開き直したか |
| Cコンパイラやlinkerが見つからない | WindowsのC++ビルド環境、LinuxのCコンパイラがあるか |
| `nagic.exe`が見つからない | ルートでコンパイラのビルドを終えたか |
| タブや字下げのエラー | インデントを空白4つに統一する |
| `Rust backend rejected program` | 直前のrustc診断を読む。Nagiの`check`より後の検査で失敗している |
| Windowsでexeを更新できない | そのアプリが実行中なら停止してから再ビルドする |
| サーバーが終了しない | `serve`はリクエストを待ち続ける。ターミナルのCtrl+Cで終了する |

次は[コードを書きながら学ぶ](language-guide.md)で、変数からAPIまで順に書いてみてください。
