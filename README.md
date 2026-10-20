# Nagi 0.2.0 — 開発中のバックエンド向け言語

[English](README.en.md)

Nagiは、Python風のHighで型付きの処理を書き、Rustのライブラリや自作コードを組み合わせるプログラミング言語です。主な対象はバックエンドです。よく使うHTTP・JSON・DB操作にはNagi APIを用意し、高度な処理はRustアダプターでつなぐ方針です。

現在はNagiからRustコードを生成し、Rust/Cargoでネイティブ実行ファイルを作ります。仕様と標準APIは開発中です。型・所有権・失敗・並行処理の扱いを実例で検証しており、`check`に成功してもRust側の検査でビルドに失敗する場合があります。

名前は日本語の「凪」に由来します。「内部は激しく動いていても、表面は凪のように穏やか」という考えを込めています。

## 書き方を読む

[準備と最初の実行](docs/getting-started.md) → [入門ガイド](docs/language-guide.md) → [最初のCLIアプリ](docs/first-app.md) → [HTTP](docs/http.md)の順で始められます。全機能の入口は[言語機能索引](docs/README.md#言語機能索引)、書式やAPIを引くには[リファレンスの目次](docs/README.md)、実例を読むには[サンプル一覧](docs/library-examples.md)へ。

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

[VS Code拡張](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang)はMarketplaceまたは[GitHub Releases](https://github.com/disnana/Nagi/releases)から入手できます。IntelliJ IDEA・PyCharm向けの[Nagi](editors/jetbrains-nagi/README.md)は[JetBrains Marketplace](https://plugins.jetbrains.com/plugin/34891-nagi)または[GitHub Releases](https://github.com/disnana/Nagi/releases)から入手できます。Marketplaceから入れる場合は、一覧のVersionsで利用可能な版とIDE対応を確認してください。GitHubの公開済み0.1.2 Releaseには、両IDE共通の`nagi-jetbrains-0.1.2.zip`と`.sha256`があります。このZIPの最低対応はIntelliJ IDEA 2025.1.1 build 251.25410.109、PyCharm 2025.1.1 build 251.25410.122です。Marketplaceからは対応IDEへ直接インストールでき、Release ZIPは手動インストールします。0.1.3は未公開候補です。公開済みcompiler 0.1.11は通常のCheck/Runに使えますが、候補の補完・診断・定義移動にはcompiler 0.2.0を使えます。操作は[エディターの案内](docs/editor.md)を参照してください。

Nagi 0.2.0は[2026-10-20に公開しました](https://github.com/disnana/Nagi/releases/tag/nagi-v0.2.0)。インストールスクリプトは最新の公開Releaseを選びます。以下のHTTP・SQLite API例には0.2.0が必要で、公開0.1.11には含まれません。`nagic --version`で導入版を確認し、更新前に[0.2.0移行ガイド](docs/migration-0.2.0.md)を参照してください。

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

## 0.2.0のHTTP例を動かす

次を`server.nagi`に保存します。DB書き込みのないHTTPサンプルです。Nagi 0.2.0で検査・実行してください。

```nagi
import std.http.server as http

class State:
    greeting: str

async def hello(request: http.Request, state: shared[State], policy: unit) -> Result[http.Response, Error]:
    return http.text(http.response_context(view(request)), http.Status.OK, view(state.greeting))

async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(greeting="Hello, Nagi!"))
    app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), hello)
    route_policy = try http.rate_policy(1000, 1000)
    app = try http.rate_route(app, http.Method.GET, "/", route_policy)
    limits = http.default_options()
    limits = try http.authority(limits, "https://localhost", ["localhost:8080", "127.0.0.1:8080"], 2, 256)
    # 明示的な有限予算の例。用途に応じて調整してください。
    response_budget = try http.response_limits(
        1048576, 8192, 32, 8192, 32, 65536, 64, 83886080, 1024, 285212672, 8192, 32
    )
    limits = try http.with_response_limits(limits, response_budget)
    rate_budget = try http.rate_limits(64, 64, 4096, 4194304, 65536)
    limits = try http.with_rate_limits(limits, rate_budget)
    return await http.serve(app, 8080, limits)

```

```sh
nagic run server.nagi
```

[http://127.0.0.1:8080/](http://127.0.0.1:8080/)を開くと`Hello, Nagi!`が返ります。Ctrl+Cで停止します。予算値は説明用であり、容量・SLAや安全な本番設定の推奨ではありません。同じrevisionのリポジトリからcompilerを作る場合は、rootで`cargo build --release --locked`を実行し、`./target/release/nagic run server.nagi`を使います。Windows PowerShellでは`.\target\release\nagic.exe run server.nagi`です。APIの変更点は[0.2.0移行ガイド](docs/migration-0.2.0.md)を参照してください。

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

## 現在の範囲（0.2.0）

| このsourceで使えるもの | 主な制限 |
|---|---|
| 型付きの値、class・enum、List、nullable、Result、move・view・shared | 利用者定義のgeneric関数・traitと標準Map操作は未対応。`owned[T]`に汎用constructorや透過的な演算はありません |
| HTTP、ヘッダー、応答status、独自エラー、共有状態 | 標準サーバーはloopbackのHTTP/1。TLS・WebSocket・streamingの公開APIはありません |
| JSON、SQLite Pool/Tx、型付きParameters、SQL/schemaのopt-in事前検査 | SQLは直接literalの一文で、placeholderは匿名`?`のみ。標準PostgreSQLはありません |
| async・scope、typed actor・Supervisor | Tokio上の同一プロセス。独自VM、無停止更新、分散actorはありません |
| ファイルのimport、標準module、Rust連携、Lowの関数差し替え | 任意のRust型をそのまま使う機能や、安定した外部ABIはありません |

SQLiteの固定SQLは直接literalを`sqlite.literal(...)`へ渡して`Query`にし、値は型付き`Parameters`でbindします。request由来文字列をSQL構造として実行するfactoryはありません。schemaを明示する[事前検査](docs/sql-check.md)は、直接QueryのSQL形・schema名・必要な列と、直接Parameters builder列のbind数を確認できます。実値の型・NULL・数値範囲はruntime検査に残り、事前検査は配備先DBとの一致を保証しません。Pool/Txの詳細は[SQLite](docs/sqlite-pool.md)を参照してください。このAPIは公開0.1.11には含まれません.


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

不具合の報告、修正、Docsや翻訳の改善を受け付けます。[貢献ガイド](CONTRIBUTING.md)に変更の相談、検証、AI利用の方針、[本体変更の手順](docs/contributing.md)にrepo mapから小さい修正・test・PRまでの入口を記載しています。脆弱性は[セキュリティ方針](SECURITY.md)の非公開報告先へ。ライセンスは[MIT](LICENSE)です。
