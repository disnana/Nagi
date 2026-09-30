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

コンパイラはRustで実装しています。HighとLowは共通の手書きparserを使い、字下げと波括弧の読み方を切り替えます。Serde JSONは診断・cost report・エディター向けのsymbol情報に使います。

ASTは式・引数・束縛名の元ソースのtoken範囲を持ちます。`symbols`は通常のcheckerと同じ規則で確認できた型を、元ファイルのUTF-16位置とともに返します。編集補助では失敗した文の変数環境を戻して次の文を解析しますが、通常の`check`は最初のエラーで失敗します。型エラーのあるコードから補完情報が得られても、ビルド可能になったことを意味しません。

定義ジャンプ用のローカル名は、型・所有権の検査とは別にASTをたどって解決します。引数・最初の代入・for・caseの束縛位置を持ち、再代入では同じ位置を保ちます。子ブロックの名前は外へ漏らさず、forの同名束縛はループ後に元へ戻します。使用位置と定義位置は既存の`references`へ出力します。move後や型の不明な初期化でも、名前の束縛先を特定できれば移動できます。VS Codeは未保存バッファも渡しますが、保存済み情報へフォールバックした場合にはF12でその位置を使いません。

codegenのscopeはJoinSet wrapperにします。classはnative structへ生成し、JSONとDB用の型付き実装を付けます。現在のCommon IRはSSAでも独自optimizerでもありません。LLVM最適化はRust backendに依存します。

ソースは1ファイル2 MBまでで、式・型・ブロックの入れ子にも上限があります。構文の変異試験と不正な入力の試験を行います。coverage-guided fuzz、incremental parsing、名前付きmodule、Rust診断をHighへ戻す精密なsource mapは今後の作業です。
