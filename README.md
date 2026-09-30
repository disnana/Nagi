# Nagi 0.1 — バックエンド向け二層言語の実行可能な試作

読みやすいHighを、編集できるLowへ変換し、ネイティブ実行ファイルにする実験です。HTTP body → 型付きclass → SQLite → class → JSONの経路が実際に動きます。性能の根拠は [PERFORMANCE.md](PERFORMANCE.md) と生の測定ログです。

この版は仕様の完成版ではありません。コンパイラとランタイムの足場を動かし、所有権・view・生成コードの差し替え・実行コストを検証するためのプロトタイプです。

## 最初に動かす

必要なものはRust/Cargo、SQLiteのCコードをビルドできるCコンパイラです。Linux x86_64で検証しました。WindowsではWSL2から同じ手順を使えます。WindowsネイティブとmacOSは未検証です。

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

primitive、値型class、連続配列、nullable、Result、関数、分岐、ループ、async/await、scope、HTTPの基本、JSON、SQLite、手書きLow呼び出しと置換を実装しています。actor・Supervisor・queueは実ランタイムとHighから呼ぶ試験用標準関数を提供します。

専用のactor宣言、汎用generic関数、trait、モジュールimport、パターンマッチ、PostgreSQL、SQLのコンパイル時検証、Highのrequest arena、Lowの生pointer/unsafe/FFI、独自scheduler、self-hostingは未実装です。`Map`と`owned`は表現方針の段階で、完全な標準APIを提供していません。

CPUの速さは、型付きネイティブ演算・boxingの回避・LLVMのループ最適化で説明できます。ランタイムはTokio/Axum/Serde/rusqliteに依存します。これらを置き換える独自ランタイムの性能を証明したものではありません。

詳細は [docs/introduction.md](docs/introduction.md)、[docs/roadmap.md](docs/roadmap.md)、[PERFORMANCE.md](PERFORMANCE.md) を参照してください。
