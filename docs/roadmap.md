# 継続開発

優先順位は、現在の動く経路を維持しながら、仕様と安全性を段階的に固めることです。

最新ソースでは、moduleの別名、独自エラーのclass・enum、Result／Option／enumのmatch、`std.http.server`、任意の所有状態を扱う`std.actor`を実装しています。新しい標準ライブラリは未リリースです。[actor](actor.md)と[Supervisor](supervisor.md)に現在の範囲を記載しています。

1. Result・Option・enumのmatchを足場に、checked/wrapping算術、文字列長、source span、変数shadowing、borrow originを確定する。
2. High checkerのpartial move・分岐・loop・escape解析を強化し、Rust backendへの依存点を縮める。
3. 登録済み標準moduleと型付きRust連携を足場に、利用者が定義するgeneric・trait、async関数を受け渡す一般の型注釈とMapの標準APIを整える。
4. `std.actor`の容量・キャンセル・再起動を検証し、Supervisor treeや独立したbounded queueの契約を固める。VM、無停止のコード差し替え、分散actorは現在の実装範囲に含まれない。
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
