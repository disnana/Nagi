# Nagi 0.1.11 — 開発中のバックエンド向け言語

[English](README.en.md)

Nagiは、Python風のHighで型付きの処理を書き、Rustのライブラリや自作コードを組み合わせるプログラミング言語です。主な対象はバックエンドです。よく使うHTTP・JSON・DB操作にはNagi APIを用意し、高度な処理はRustアダプターでつなぐ方針です。

現在はNagiからRustコードを生成し、Rust/Cargoでネイティブ実行ファイルを作ります。仕様と標準APIは開発中です。型・所有権・失敗・並行処理の扱いを実例で検証しており、`check`に成功してもRust側の検査でビルドに失敗する場合があります。

名前は日本語の「凪」に由来します。「内部は激しく動いていても、表面は凪のように穏やか」という考えを込めています。

## 書き方を読む

[準備と最初の実行](docs/getting-started.md) → [入門ガイド](docs/language-guide.md) → [HTTP](docs/http.md)の順で始められます。書式やAPIを引くには[リファレンスの目次](docs/README.md)、実例を読むには[サンプル一覧](docs/library-examples.md)へ。

紹介と日英のDocsは[公式サイト](https://nagi.disnana.com/)で読めます。[目的と実装の範囲](docs/introduction.md)、[設計判断とその理由](DESIGN.md)、[今後の優先順位](docs/roadmap.md)も記載しています。

AIコーディング向けのDocsと開発skillは、人間向けの`docs/`とは分けて[ai/](ai/README.md)に置いています。

## インストール

NagiアプリのビルドにはRust/CargoとCのビルド環境が必要です。[OSごとの準備](docs/getting-started.md)を確認してください。コンパイラのインストールはビルド済みの配布物を取得します。

Windows（PowerShell）:

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.ps1')))
```

Linux / macOS（bash）:

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.sh | bash) && export PATH="$HOME/.local/bin:$PATH"
```

最新の[GitHub Release](https://github.com/disnana/Nagi/releases)を取得し、SHA-256を確認してPATHに登録します。更新も同じコマンドです。[版の指定・旧版の整理](docs/getting-started.md#更新する)は導入ガイドに記載しています。自分で展開する場合はアーカイブ全体を展開し、`runtime/`を含むフォルダーをPATHに追加してください。

```sh
nagic --version
nagic --help
```

[VS Code拡張](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang)と[IntelliJ IDEA・PyCharm向けプラグインとRelease ZIPの導入方法](editors/jetbrains-nagi/README.md)もあります。操作は[エディターの案内](docs/editor.md)を参照してください。

インストーラーはmainから、コンパイラは公開Releaseから取得します。このREADMEの例はNagi 0.1.11を対象にしています。0.1.11の公開有無は公式Release記録で確認し、インストールした版は`nagic --version`で確かめてください。変更内容は[CHANGELOG](CHANGELOG.md)、互換性の変更は[0.1.11移行ガイド](docs/migration-0.1.11.md)を参照してください。

### アンインストール

Windows（PowerShell）:

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/uninstall.ps1')))
```

Linux / macOS（bash）:

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/uninstall.sh | bash)
```

削除予定を確認するにはPowerShellのコマンド末尾に`-WhatIf`を付けるか、bash側を`bash -s -- --dry-run`にします。独自の導入先を使った場合は、インストール時と同じ`-InstallDir`または`--prefix`・`--bin-dir`を指定し、独自のプロファイルには同じ`--profile`を指定してください。

変更済み・検証できない配布物は残します。プロジェクト、Rust/Cargo、VS Code拡張は変更しません。詳細は[導入ガイド](docs/getting-started.md#アンインストールする)へ。

## 最初に動かす

次を`server.nagi`に保存します。DBなしのHTTPサーバーです。

```python
import std.http.server as http

class State:
    greeting: str

async def hello(request: http.Request, state: shared[State]) -> Result[http.Response, Error]:
    return ok(http.text(http.Status.OK, view(state.greeting)))

async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(greeting="Hello, Nagi!"))
    app = try http.route(app, http.Method.GET, "/", hello)
    return await http.serve(app, 8080, http.default_options())
```

```sh
nagic run server.nagi
```

[http://127.0.0.1:8080/](http://127.0.0.1:8080/)を開くと`Hello, Nagi!`が返ります。Ctrl+Cで停止します。ソースからコンパイラを作る場合は、`cargo build --release --locked`の後に`./target/release/nagic`を使います。

ヘッダー、JSON、独自エラー、共有状態の使い方は[HTTP](docs/http.md)と[認証サンプル](test-nagi-code/library-examples/http-auth/README.md)へ。

HTTPの基盤をRust側に置くこともできます。[Axum見積API](test-nagi-code/application-examples/axum-service/README.md)では、AxumがrouteとJSON入力を担当し、Nagiのasync関数が型付きの検証と価格計算を行います。手書きRustとの分担と制約は[設計方針](DESIGN.md#rust資産との接続を中心にする)にまとめています。

## コード構造を図にする

`nagic map`で型・モジュール・関数呼び出しを調べ、Mermaid、D2、JSON、単一HTMLへ出力できます。

```sh
nagic map calls --project examples/code-map --format html --output calls.html
```

SVG・PNGには別途D2が必要です。絞り込み、推論できない関係、描画方法は[コードマップ](docs/code-map.md)を参照してください。

## HighとLow

通常は字下げで書くHigh（`.nagi`）を使います。Low（`.low`）は同じ型・所有権の規則を使う波括弧の構文で、生成コードの確認や関数の差し替えに使えます。生ポインター、unsafe、配置指定、C ABIは未対応です。

Lowの互換性を保ちながら、当面はHighとRust連携を優先します。使い方は[HighとLow](docs/low-language.md)、Rustの資産を呼ぶ方法は[Rust連携](docs/modules-and-rust.md)へ。

## 現在の範囲

| このソースで使えるもの | 主な制限 |
|---|---|
| 型付きの値、class・enum、List、nullable、Result、move・view・shared | 利用者が定義するgeneric・trait、Mapの標準操作は未対応。ownedの扱いは未完成 |
| HTTP、ヘッダー、応答status、独自エラー、共有状態 | 標準サーバーはloopbackのHTTP/1。TLS・WebSocket・streamingの公開APIはない |
| JSON、SQLite、schemaを指定したSQLの事前検査 | 事前検査は明示指定。値の型・NULL・動的SQLは実行時検査。bindは固定形。標準PostgreSQL・pool・transaction APIはない |
| async・scope、typed actor・Supervisor | Tokio上の同一プロセス。独自VM、無停止更新、分散actorはない |
| ファイルのimport、標準module、Rust連携、Lowの関数差し替え | 任意のRust型をそのまま使う機能や、安定した外部ABIはない |

SQLiteの文字列SQLは、schemaを明示した[事前検査](docs/sql-check.md)で名前・必要な返却列・bind数を確認できます。値の型・NULL可否や配備先schemaの一致は保証しません。公開版への収録状況は[CHANGELOG](CHANGELOG.md)を参照してください。

各APIの条件は[リファレンス](docs/README.md)で確認してください。Nagiの検査とrustcの役割は[所有権](docs/ownership.md)、実装基盤の分担は[紹介](docs/introduction.md#コンパイラと既存ライブラリ)に記載しています。

性能値は[測定条件と生ログ](PERFORMANCE.md)の範囲に限ります。Rust＋Axumより高速・成熟しているとする結論はありません。

## 構成

| 場所 | 内容 |
|---|---|
| `compiler/` | lexer、High/Low parser、型・move・view検査、Rust生成、CLI |
| `runtime/` | HTTP、JSON、SQLite worker、scope、actor、Supervisor |
| `examples/`・`test-nagi-code/` | 入門例、アプリ、Rust連携、Low差し替えの例 |
| `editors/` | VS Code・JetBrains向けプラグイン |
| `docs/`・`website/` | 日英リファレンス、設計案、公式サイト |
| `tests/`・`scripts/`・`benchmarks/` | 検証、配布、測定と生ログ |

## 検証を再現する

コンパイラとランタイムの基本確認:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --examples --bins --locked
python3 scripts/build_examples.py
```

変更ごとの追加確認は[CONTRIBUTING](CONTRIBUTING.md#手元で確認する)、HTTP・性能測定は[検証方法](docs/performance.md)、サイトは[ビルドとリンク検査](website/README.md#手元で確認する)を参照してください。

処理系の契約と、High・Low・生成Rustを通す回帰・生成テストは[compiler testing](docs/internal/compiler-testing.md)にまとめています。リポジトリを開発するAI向けの指示は[AGENTS.md](AGENTS.md)です。

## 貢献・ライセンス

不具合の報告、修正、Docsや翻訳の改善を受け付けます。[貢献ガイド](CONTRIBUTING.md)に変更の相談、検証、AI利用の方針を記載しています。脆弱性は[セキュリティ方針](SECURITY.md)の非公開報告先へ。ライセンスは[MIT](LICENSE)です。
