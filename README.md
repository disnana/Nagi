# Nagi 0.1 — バックエンド向け二層言語の実行可能な試作

読みやすいHighを、編集できるLowへ変換し、ネイティブ実行ファイルにする実験です。HTTP body → 型付きclass → SQLite → class → JSONの経路が実際に動きます。性能の根拠は [PERFORMANCE.md](PERFORMANCE.md) と生の測定ログです。

この版は仕様の完成版ではありません。コンパイラとランタイムの足場を動かし、所有権・view・生成コードの差し替え・実行コストを検証するためのプロトタイプです。

## 書き方を読む

**[ドキュメントの目次](docs/README.md) → [準備と最初の実行](docs/getting-started.md) → [コードを書きながら学ぶ](docs/language-guide.md)** の順で進められます。書式を引くには[文法の早見表](docs/syntax.md)、引数を調べるには[組み込み関数](docs/builtins.md)、サイトやAPIを作るには[HTTPの入門](docs/http.md)を参照してください。動く入門例は[examples/tutorial/](examples/tutorial/)にあります。

## 最初に動かす

必要なものはRust/Cargo、SQLiteのCコードをビルドできるCコンパイラです。Linux x86_64とWindowsネイティブでビルド・実行を確認しています。WindowsのMSVC環境とPowerShellのコマンドは[準備と最初の実行](docs/getting-started.md)にあります。WSL2では次のLinux手順を使えます。macOSは未検証です。

```bash
cargo build --release --locked
./target/release/nagic run examples/hello.nagi
./target/release/nagic run examples/values.nagi
./target/release/nagic run examples/crud.nagi --cost-report
```

別のターミナルでAPIを呼び出します。

```bash
curl http://127.0.0.1:8080/health
curl -H 'Content-Type: application/json' -d '{"name":"tp-li","age":18}' http://127.0.0.1:8080/users
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
| `tests/` | 実HTTP通信の統合テスト |
| `fuzz/` | seed固定のparser/JSON mutation試験 |
| `benchmarks/` | 比較実装、wrkスクリプト、生ログ |
| `docs/` | 言語仕様、設計理由、実装範囲、継続開発の指針 |
| `scripts/` | ビルド・実通信・負荷・結果生成の再現スクリプト |

High/Lowは同じcompiler crate内の別経路です。独立crateへの分割とstdライブラリのモジュール化は今後の作業です。

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

## 現在の範囲

primitive、値型class、連続配列、nullable、Result、関数、分岐、ループ、async/await、scope、HTTPの基本、HTML応答、JSON、SQLite、手書きLow呼び出しと置換、相対ファイルのimport、型付きRust関数の呼び出しを実装しています。actor・Supervisor・queueは実ランタイムとHighから呼ぶ試験用標準関数を提供します。ファイル分割とRustのcrate利用は [docs/modules-and-rust.md](docs/modules-and-rust.md)、画面付きデモは [test-nagi-code/web-demo/README.md](test-nagi-code/web-demo/README.md) を参照してください。

Resultの`match`で成功・失敗を分け、既定値に回復したり、Errorの種類を保って返したりできます。[書き方](docs/error-handling.md)と[実HTTPで試すAPIサンプル](test-nagi-code/result-api/README.md)を用意しています。VS Code拡張0.1.2では関数・class・import先へのF12も使えます。

[VS Code拡張0.1.3](editors/vscode-nagi/README.md)では、関数・classの宣言の型表示、引数を挿入する補完、呼び出し時の引数ヒントを利用できます。未保存のソースを読み込み、書きかけの構文では保存済みの宣言を表示して補助します。

専用のactor宣言、汎用generic関数、trait、名前付きmoduleとalias、nullableや一般的なパターンのmatch、PostgreSQL、SQLのコンパイル時検証、Highのrequest arena、Lowの生pointer/unsafe/C ABI、独自scheduler、self-hostingは未実装です。Rust連携は同じビルド内の呼び出しで、安定した外部ABIではありません。`Map`と`owned`は表現方針の段階で、完全な標準APIを提供していません。

CPUの速さは、型付きネイティブ演算・boxingの回避・LLVMのループ最適化で説明できます。ランタイムはTokio/Axum/Serde/rusqliteに依存します。これらを置き換える独自ランタイムの性能を証明したものではありません。

詳細は [docs/introduction.md](docs/introduction.md)、[docs/roadmap.md](docs/roadmap.md)、[PERFORMANCE.md](PERFORMANCE.md) を参照してください。
