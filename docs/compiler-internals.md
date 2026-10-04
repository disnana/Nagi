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

Nagi側の型・所有権・view検査は、Cargoを起動せず、`check`やエディターでNagiの規則とソース位置に基づく診断を返すためにあります。生成Rustはrustcが最終検査します。両者の整合性を維持する必要があり、`check`の成功は`build`の成功を保証しません。Rust側の借用・Send条件、手書きRustの本体、crateのAPIなどはビルドで検査します。

コンパイラはRustで実装しています。HighとLowは共通の手書きparserを使い、字下げと波括弧の読み方を切り替えます。相対ファイルのmoduleと定義にIDを持たせ、別名やLowへの変換を経ても同じ定義を参照します。Serde JSONは生成Lowのmodule情報、診断・cost report・エディター向けのsymbol情報に使います。

ASTは式・引数・束縛名の元ソースのtoken範囲を持ちます。`symbols`は通常のcheckerと同じ規則で確認できた型を、元ファイルのUTF-16位置とともに返します。編集補助では失敗した文の変数環境を戻して次の文を解析しますが、通常の`check`は最初のエラーで失敗します。型エラーのあるコードから補完情報が得られても、ビルド可能になったことを意味しません。

定義ジャンプ用のローカル名は、型・所有権の検査とは別にASTをたどって解決します。引数・最初の代入・for・caseの束縛位置を持ち、再代入では同じ位置を保ちます。子ブロックの名前は外へ漏らさず、forの同名束縛はループ後に元へ戻します。使用位置と定義位置は既存の`references`へ出力します。move後や型の不明な初期化でも、名前の束縛先を特定できれば移動できます。VS Codeは未保存バッファも渡しますが、保存済み情報へフォールバックした場合にはF12でその位置を使いません。

codegenのscopeはJoinSet wrapperにします。classはnative structへ生成し、JSONとDB用の型付き実装を付けます。Lowはコンパイル時の共通表現で、実行時VMではありません。現在のCommon IRはSSAでも独自optimizerでもありません。LLVM最適化はRust backendに依存します。

loweringとcodegenは、生成行と元の文・定義・フィールドの行の対応を保持します。Lowは独立して再解析し、統合前に診断の行番号を元へ戻します。ビルドではCargoのJSON診断を読み、対応するNagi・Lowのファイルと行を先に表示します。Rustの補足や修正候補は生成Rustの座標のまま残し、手書きRustや位置の不明な診断は書き換えません。

ソースは1ファイル2 MBまでで、式・型・ブロックの入れ子にも上限があります。構文の変異試験と不正な入力の試験を行います。coverage-guided fuzz、incremental parsing、引用符なしの標準module、式の厳密な列位置や全Rust診断を扱うsource mapは今後の作業です。
