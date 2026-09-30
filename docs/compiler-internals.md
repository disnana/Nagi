# コンパイラの構成

| 段階 | 実装 |
|---|---|
| lexer | line/column、字下げ、文字列、数値、operatorをtoken化 |
| High parser | 字下げブロックとPratt expression parser |
| High checker | ローカル推論、型整合性、move、view origin、async/Result/scope |
| lowering | 推論済み型とlet宣言を含むLowテキストへ出力 |
| Low parser | 波括弧・セミコロンのLowを独立して解析 |
| native統合 | 通常関数追加、@replaceのsignature検査 |
| Common IR | 型の付いたLow ASTを共通表現として利用 |
| codegen | 安全なRust、Serde/FromRow/HTTP wrapperへ生成 |
| backend | rustc/Cargoでネイティブを生成。借用・Sendも検査 |

最初のcompilerはRustです。parserは小さい構文を追える手書き実装にし、構文が安定する前の依存を減らしました。Serde JSONは診断・cost report用です。

codegenのscopeはJoinSet wrapperにします。classはnative structへ生成し、JSONとDB用の型付き実装を付けます。現在のCommon IRはSSAでも独自optimizerでもありません。LLVM最適化はRust backendに依存します。

sourceは2 MB、式・型・ブロックの入れ子に上限を設けています。mutation smokeとmalformed sourceを試験します。coverage-guided fuzz、incremental parsing、複数module、精密なHigh/Low source mapは今後の作業です。
