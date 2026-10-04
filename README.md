# Nagi 0.1.9 — バックエンド向け二層言語の実行可能な試作

[English](README.en.md)

読みやすいHighを、編集できるLowへ変換し、ネイティブ実行ファイルにする実験です。HTTP body → 型付きclass → SQLite → class → JSONの経路が実際に動きます。性能の根拠は [PERFORMANCE.md](PERFORMANCE.md) と生の測定ログです。

この版は仕様の完成版ではありません。コンパイラとランタイムの足場を動かし、所有権・view・生成コードの差し替え・実行コストを検証するためのプロトタイプです。

名前は日本語の「凪」に由来します。「内部は激しく動いていても、表面は凪のように穏やか」という考えを込めています。

## 書き方を読む

**[ドキュメントの目次](docs/README.md) → [準備と最初の実行](docs/getting-started.md) → [コードを書きながら学ぶ](docs/language-guide.md)** の順で進められます。書式を引くには[文法の早見表](docs/syntax.md)、引数を調べるには[組み込み関数](docs/builtins.md)、サイトやAPIを作るには[HTTPの入門](docs/http.md)を参照してください。動く入門例は[examples/tutorial/](examples/tutorial/)にあります。CLI、HTTP、SQLite、Supervisorを試す[7つのアプリ](test-nagi-code/application-examples/README.md)も用意しています。

紹介と日英の全Docsは[公開サイト](https://nagi.disnana.com/)で読めます。[English](https://nagi.disnana.com/en/)もあります。ソースとPagesの自動公開手順は[`website/`](website/README.md)にあります。

配布物は[GitHub Releases](https://github.com/disnana/Nagi/releases)に掲載します。mainでNagiまたはVS Code拡張のバージョンを上げると、CI成功後にその配布物を正式リリースします。[運用手順](scripts/releases/README.md)に条件と成果物をまとめています。

## インストール

mainには次回配布予定の修正も含まれます。公開版との差分は[変更履歴](CHANGELOG.md)を参照してください。

NagiアプリのビルドにはRust/CargoとCのビルド環境が必要です。準備は[最初の実行](docs/getting-started.md)を参照してください。

Windows（PowerShell）:

```powershell
& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.ps1')))
```

Linux / macOS（bash）:

```bash
(set -o pipefail; curl -fsSL https://raw.githubusercontent.com/disnana/Nagi/main/scripts/install.sh | bash) && export PATH="$HOME/.local/bin:$PATH"
```

最新の公開版を取得し、SHA-256を確認してユーザー用の場所へ展開し、PATHに登録します。**更新も同じコマンドです。** インストーラーはmainから読みますが、コンパイラはGitHub Releasesの正式な配布物を使います。RustやVS Code拡張は別途用意します。

VS Code拡張は[Marketplace](https://marketplace.visualstudio.com/items?itemName=Disnana.nagi-lang)からインストールできます。設定と使い方は[拡張の案内](editors/vscode-nagi/README.md)を参照してください。

IntelliJ IDEA・PyCharm向けの初期プラグインもあります。High／Lowの色付け、インデント補助、型検査・実行に対応します。[ビルドとインストール](editors/jetbrains-nagi/README.md)を参照してください。

更新前に実行中のNagiのビルドを止め、更新後はVS Codeを再起動してください。更新が成功したら、配布時の内容と一致する旧版を削除し、使用する1版だけ残します。追加・変更されたファイルは保護します。照合不能や使用中などで削除できない旧版も残し、その場所を表示します。失敗時は元のコマンドを維持します。[版の指定と更新の詳細](docs/getting-started.md#更新する)も参照してください。

```bash
nagic --version
nagic --help
```

自分で展開する場合は、[GitHub Releases](https://github.com/disnana/Nagi/releases)のOS別アーカイブ全体を展開し、展開フォルダーをPATHに追加します。`runtime/`を同じ場所に保てば、`NAGI_ROOT`の設定は不要です。

## 最初に動かす

以下はソースからコンパイラとサンプルをビルドする手順です。インストール済みのコンパイラでは`nagic run <ファイル>`を使います。

```bash
cargo build --release --locked
./target/release/nagic run examples/hello.nagi
./target/release/nagic run examples/values.nagi
./target/release/nagic run examples/crud.nagi --cost-report
```

別のターミナルでAPIを呼び出します。

```bash
curl http://127.0.0.1:8080/health
curl -H 'Content-Type: application/json' -d '{"name":"alice","age":18}' http://127.0.0.1:8080/users
curl http://127.0.0.1:8080/users/1
```

```python
class User:
    id: i64
    name: str
    age: i32

@get("/users/{id}")
async def get_user(db: Db, id: i64) -> Result[User?, Error]:
    return await db_query[User](db, "SELECT id, name, age FROM users WHERE id = ?1", id)
```

Pythonコードとして実行する構文ではありません。`nagic`でビルドしてください。

複数ファイルのアプリは[nagi.toml](docs/projects.md)に入口・Rust依存・手書きLowの設定を保存できます。たとえば `./target/release/nagic run --project test-nagi-code/rust-bridge` でRust連携サンプルを動かせます。[VS Code拡張](editors/vscode-nagi/README.md)も同じ設定を使い、補助ファイルから入口の検査・ビルド・実行を行います。

## コード構造を図にする

`nagic map`で型・モジュール・関数呼び出しを調べ、Mermaid、D2、JSON、単一HTMLへ出力できます。D2がPATHにあればSVG・PNGも生成します。

```bash
nagic map types --project examples/code-map --format d2
nagic map calls --project examples/code-map --format html --output calls.html
```

絞り込みと描画方法は[コードマップ](docs/code-map.md)を参照してください。

## HighとLow

```bash
./target/release/nagic lower examples/override.nagi
./target/release/nagic run examples/override.nagi --native examples/native/override.low
./target/release/nagic run examples/low_call.nagi --native examples/native/math.low
./target/release/nagic run examples/hello.low
```

`build/<name>/generated.low`は読み返せるLowソースです。Lowは独立して解析・型検査します。生成物への変更を残す場合は手書きLowへ関数を移し、`@replace generated::関数名`で指定します。再生成は手書きファイルを変更しません。

## 構成

| 場所 | 内容 |
|---|---|
| `compiler/` | Rust製lexer、High/Low parser、型・move・view検査、Low統合、Rust codegen、CLI |
| `runtime/` | HTTP、JSON、SQLite専用worker、scope、actor、Supervisor、queue、計測 |
| `examples/` | High、Low、手書き置換の実行サンプル |
| `test-nagi-code/` | タスク管理サイト、在庫API、Resultの回復処理、Rust連携のサンプル |
| `editors/vscode-nagi/` | VS Codeの色付け・診断・F12・型ホバー・補完・引数ヒント・check/build/run拡張 |
| `editors/jetbrains-nagi/` | IntelliJ IDEA・PyCharm向けの色付け・インデント・折りたたみ・check/run拡張 |
| `tests/` | 実HTTP通信の統合テスト |
| `fuzz/` | seed固定のparser/JSON mutation試験 |
| `benchmarks/` | 比較実装、wrkスクリプト、生ログ |
| `docs/` | 言語仕様、設計理由、実装範囲、継続開発の指針 |
| `scripts/` | ビルド・実通信・負荷・結果生成の再現スクリプト |

High/Lowは同じcompiler crate内の別経路です。`std.http.server`と`std.actor`は登録済みの標準ライブラリとして提供します。独立crateへの分割は今後の作業です。

[ライブラリとRust連携のサンプル](docs/library-examples.md)では、同じ共通コードを使うCLI・JSONレポート、serde_json、Tokio、HTTP、Supervisor、Lowの差し替えを試せます。[自作基盤の構成](docs/libraries.md)と[今後の設計案](docs/library-design.md)も公開しています。

## 検証を再現する

```bash
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --release --examples --bins --locked
python3 scripts/build_examples.py
python3 -m pip install -r tests/requirements.txt
python3 tests/http_integration.py
./target/release/examples/fuzz-smoke
./native-target/release/nagi-cpu
./target/release/examples/microbench
./target/release/examples/concurrency_bench
python3 benchmarks/python_cpu.py
node benchmarks/node_cpu.js
python3 tests/connections.py
python3 scripts/http_bench.py --wrk /absolute/path/to/wrk --soak 120
python3 scripts/summarize_results.py
```

サーバーはloopbackへbindします。試験時に8080/8081/8082/8083を空けてください。DBを永続化する場合は`NAGI_DB=users.sqlite`を指定します。HTTP executorのworker数は`NAGI_THREADS`、標準は4です。比較試験では全サーバーを1論理CPUに固定します。

接続数の限界、長時間の負荷、負荷が止まった後の回復は[通信の負荷試験](docs/http-capacity.md)で確認できます。

## 現在の範囲

primitive、値型class、enum、連続配列、nullable、独自エラー型のResult、関数、分岐、ループ、async/await、scope、HTTPの基本、HTML応答、JSON、SQLite、手書きLow呼び出しと置換、相対ファイルのimport、型付きRust関数の呼び出しを実装しています。旧actor・Supervisor・queueの組み込み関数は検証用APIです。ファイル分割とRustのcrate利用は [docs/modules-and-rust.md](docs/modules-and-rust.md)、画面付きデモは [test-nagi-code/web-demo/README.md](test-nagi-code/web-demo/README.md) を参照してください。

Resultの`match`で成功・失敗を分け、既定値に回復したり、Errorの種類を保って返したりできます。[書き方](docs/error-handling.md)と[実HTTPで試すAPIサンプル](test-nagi-code/result-api/README.md)があります。

[VS Code拡張0.1.12](editors/vscode-nagi/README.md)では、関数・class・import先・ローカル変数へのF12、宣言とローカル変数の型ホバー、classのフィールド補完、呼び出し時の引数ヒントを利用できます。一度保存したファイルの未保存の編集にも対応します。[操作例](docs/editor.md)で、型の表示や`value.`からの補完、定義への移動を試せます。

Nagi 0.1.8から使える[標準HTTP module](docs/http.md)では、DBなしのApp、ヘッダー、Method／Status、独自の状態とエラー処理を使えます。標準moduleのimportとOptionのSome／None分岐にも対応します。公開済み版との差は[変更履歴](CHANGELOG.md)を参照してください。

Nagi 0.1.8から使える[`std.actor`](docs/actor.md)では、通常のasync関数で任意の所有状態を扱い、型付きメッセージ・返信、再起動方針、監視、停止を使えます。[API](docs/actor-reference.md)と[サンプル](test-nagi-code/library-examples/supervised-service/README.md)を用意しています。同じプロセス内のnative実装で、BEAMのようなVM、無停止のコード差し替え、分散actorは未対応です。

専用のactor宣言、利用者が定義するgeneric関数やtrait、パッケージのimport、一般的なパターンのmatch、PostgreSQL、SQLのコンパイル時検証、Highのrequest arena、Lowの生pointer/unsafe/C ABI、独自scheduler、self-hostingは未実装です。Rust連携は同じビルド内の呼び出しで、安定した外部ABIではありません。`Map`と`owned`の操作APIも揃っていません。

CPUの速さは、型付きネイティブ演算・boxingの回避・LLVMのループ最適化で説明できます。ランタイムはTokio/Axum/Serde/rusqliteに依存します。これらを置き換える独自ランタイムの性能を証明したものではありません。

詳細は [docs/introduction.md](docs/introduction.md)、[docs/roadmap.md](docs/roadmap.md)、[PERFORMANCE.md](PERFORMANCE.md) を参照してください。

## 貢献とライセンス

不具合の報告、コードの修正、Docsや翻訳の改善を受け付けます。手順とAI利用の方針は[貢献ガイド](CONTRIBUTING.md)、脆弱性の報告方法は[セキュリティ方針](SECURITY.md)にあります。Nagiは[MITライセンス](LICENSE)で公開しています。
