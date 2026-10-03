# 継続開発

優先順位は、現在の動く経路を維持しながら、仕様と安全性を段階的に固めることです。

1. Resultのmatchを足場に、nullableのmatch、checked/wrapping算術、文字列長、source span、変数shadowing、borrow originを確定する。
2. High checkerのpartial move・分岐・loop・escape解析を強化し、Rust backendへの依存点を縮める。
3. 相対ファイルのmodule・aliasと型付きRust連携を足場に、引用符なしの標準module、汎用generic、trait、async関数を受け渡す型注釈とMapの標準APIを実装する。
4. 任意stateのactor宣言、Supervisor tree、bounded queue宣言を現在のランタイムへlowerする。
5. request arenaとborrowed class、DBでstep中にencodeする経路、buffer再利用、streaming JSONを比較測定する。
6. 型付きSQL引数・行・transactionの共通契約を固め、既存Rust driverを使ってPostgreSQLへ対応する。キャンセルとpoolの終了を実DBで確認する。
7. Lowのlayout・pointer・arena・unsafe境界・C ABIを定義し、sanitizer/coverage-guided fuzzを整える。
8. Rust codegenから独立backendを導入する。schedulerを置き換える判断も測定で行う。

## self-hosting

| Stage | 内容 | 状況 |
|---|---|---|
| 0 | RustでLow compiler | 小さい言語subsetを実装 |
| 1 | RustでHigh compiler | Lowへの変換を実装 |
| 2 | LowでLow compilerを書き直す | 未着手。String/Map/module/allocator APIの拡張が必要 |
| 3 | Low compilerが自身をコンパイル | 未着手。bootstrap比較とdeterminism試験が必要 |

実用化には複数の開発段階が必要です。この試作のテストが通ったことは、本番用言語・runtimeの完成や、全unsafe/FFI経路の安全性を意味しません。工数の正確な見積もりは仕様と担当体制が決まってから行います。

自作基盤とRust資産の再利用については、[動くサンプル](library-examples.md)と[具体的な設計案](library-design.md)を用意しています。依存設定、名前空間、resourceとDBの契約を、既存コードを保ちながら段階的に整える案です。
