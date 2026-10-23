# Nagiの良さと、他の言語との比較

Nagiは、**読みやすい字下げ構文で型付きのバックエンド処理を書き、Rustの実行基盤とライブラリを使いたい**開発者に向けた言語です。公開版0.2.1では、HTTP・JSON・SQLite、Result、所有権・借用、async・actorを使えます。0.2.2は開発中です。

## Nagiで得られるもの

| 良さ | 開発でどう役立つか | 試す・根拠を読む |
|---|---|---|
| 字下げ構文と明示的な型・失敗 | 業務処理の入力・出力・失敗を短い関数にまとめ、読む人やAIが境界を追いやすい | [最初のCLIアプリ](first-app.md)、[Result](error-handling.md) |
| Rustコンパイル前の検査 | 編集後は`nagic check`でNagiの構文・型・所有権の問題を先に見つける | [開発の早見表](developer-workflow.md) |
| ネイティブ実行とRust資産 | Rustを生成して実行ファイルを作る。必要なcrateは型付きの小さなadapterで使える | [Rust連携](modules-and-rust.md)、[Axumアプリ](../test-nagi-code/application-examples/axum-service/README.md) |
| 境界を明示する標準API | HTTPの予算、認証・認可、SQLのbindなどをAPIと型に表す | [HTTP](http-server.md)、[認証](security.md)、[SQLite](sqlite-pool.md) |
| 開発用のビルド経路 | dev profile、共有Cargo依存、変更の観測、明示指定の生成本体・関数群の再利用 | [開発ビルド](development-builds.md)、[アプリCI](user-ci.md) |
| AIが参照できる開発手順 | 対応構文・診断・検証を追うSkillと用途別資料を使う | [AI向けDocs](../ai/README.md) |

例えば、型付きの計算は次のように書けます。この例は`42`を表示します。

```nagi
def double(value: i64) -> i64:
    return value * 2

def main():
    print(double(21))
```

これはPython互換コードではありません。Nagiの型・move/view・Resultの規則を使います。HTTP、DB、非同期処理の実例は[動くサンプル](library-examples.md)から選べます。

## Python/FastAPI・Rust/Axum・Goとの違い

以下は開発方式の比較です。言語とフレームワークを組み合わせる場合も含み、速度ランキングではありません。

| 判断軸 | Nagi 0.2.1 | Python + FastAPI | Rust + Axum | Go + `net/http` |
|---|---|---|---|---|
| コードの書き方 | 字下げ構文、関数の型、Result、move/view | 字下げ構文、型注釈とPydanticによる入力モデル | Rustの型・所有権・trait、Result、async | 明示的な型、構造体・interface、error、goroutine |
| 編集後の確認 | Nagiの完全`check`、必要時にCargoビルド | Python実行、型検査ツール、テスト。通常はアプリのネイティブビルド不要 | `cargo check`と`cargo test`/実行 | `go test`と`go run`/`go build` |
| 配布と実行 | 生成Rustをコンパイルしたネイティブ実行ファイル。対象OS・native依存を確認 | Python実行環境、アプリ依存、ASGIサーバー | ネイティブ実行ファイル。対象OS・native依存を確認 | ネイティブ実行ファイル。cgo使用時などはnative依存を確認 |
| メモリの扱い | Nagiの所有権検査とRustの最終検査。共有APIはArc等を使う | オブジェクトを処理系が管理。CPythonには参照カウントと循環GCがある | 所有権・借用、必要に応じArc等 | GCとgoroutineのruntime |
| ライブラリ | Nagi標準APIと明示的なRust adapter。Python packageは直接使えない | PyPIとPythonの既存ライブラリ | crates.ioとRustライブラリ | Go modulesと標準ライブラリ |
| 現在の選びどころ | 小さなバックエンド・CLI、型付き業務処理とRust資産を組み合わせる試行 | Pythonの既存資産を重視するAPI・データ処理 | Rust全体の表現力・制御・エコシステムを使うサービス | Goの既存資産、標準ツールと並行処理を使うサービス |

他言語の詳細は公式資料を参照してください：[FastAPIの型と検証](https://fastapi.tiangolo.com/python-types/)、[Pythonのメモリ管理](https://docs.python.org/3/c-api/memory.html)、[Rustの所有権](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)、[Cargo check](https://doc.rust-lang.org/cargo/commands/cargo-check.html)、[Axum](https://docs.rs/axum/latest/axum/)、[Goコマンド](https://pkg.go.dev/cmd/go)、[Go HTTP](https://pkg.go.dev/net/http)、[Go GC](https://go.dev/doc/gc-guide)。各言語のすべての処理系・設定を代表する表ではありません。

## どの用途で試すか

まずは、[JSONの検証と集計](../test-nagi-code/application-examples/stock-report/README.md)、[SQLiteを使う設定保存](../test-nagi-code/application-examples/device-settings/README.md)、[Rust/Axumから呼ぶ業務関数](../test-nagi-code/application-examples/axum-service/README.md)のように、入力・出力と必要なAPIが明確な処理を選べます。正常入力と拒否入力をテストし、必要な機能が標準APIで足りるか、adapterが必要かを確認します。

現在はmethod・継承・汎用trait・標準PostgreSQL APIなどに未対応です。PythonライブラリやRustの型をそのままNagiへ持ち込むことはできません。大規模運用の採用実績や他言語と同等の成熟度も、このページでは主張しません。[文法](syntax.md)・[実装範囲](introduction.md)・[予定](roadmap.md)を分けて確認できます。

## 速度は条件と一緒に見る

Rustからネイティブコードを作ることだけで、手書きRustと同じ性能やPython/Goに対する倍率は決まりません。HTTP、JSON、DB、allocation、clone、非同期処理などの条件を揃えて測る必要があります。

既存の[HTTP測定](http-stdlib-performance.md)、[ビルド比較](../benchmarks/results/smart-build-2026-10-20/README.md)、[関数分割の測定](../benchmarks/results/function-unit-reuse-2026-10-21/README.md)には条件と結果があります。関数分割は変更なしで改善する一方、編集時に遅くなったケースもあります。初回ビルドにはCargo依存のコンパイルが必要で、Pythonの起動時間と同じ条件にはなりません。

Nagi・FastAPI・Axum・Goを同一仕様で比較した総合ベンチマークは、ここでは提示していません。採用時は[測定方法](performance.md)に沿って、自分のアプリのビルド時間、応答時間、メモリ、必要なコードとテストを比較してください。
