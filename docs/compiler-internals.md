# コンパイラの構成

| 段階 | 実装 |
|---|---|
| lexer | line/column、字下げ、文字列、数値、operatorをtoken化 |
| High parser | 字下げブロックとPratt expression parser |
| High checker | ローカル推論、型整合性、move、view origin、async/Result/scope |
| lowering | 推論済み型とlet宣言を含むLowテキストへ出力 |
| Low parser / checker | 波括弧・セミコロンのLowを再解析し、型・所有権を再検査 |
| native統合 | 通常関数追加、@replaceのsignature検査 |
| Common IR | HighとLowで共有するProgram ASTを利用 |
| codegen | Rustと、対応する型のSerde/FromRow/HTTP wrapperへ生成 |
| backend | rustc/Cargoでネイティブを生成。借用・Sendも検査 |

Nagi自身が構文解析、型検査、Rustコード生成を担当し、Cargo/rustcが依存のビルド、最終的な借用・trait検査、機械語生成を担当します。独自の機械語backendやVMはありません。

HighとLowは共通の手書きparserとProgram ASTを使い、字下げと波括弧の読み方を切り替えます。通常のHighの処理経路はHighを検査し、Lowテキストへ出力して再解析し、手書きLowと統合して再検査します。Lowは別のメモリモデルを持つ層ではありません。

Lowには型注釈・制御構造と、module・定義のIDを残します。型推論の結果や名前解決、move・借用の検査状態を証明として持ち越すわけではなく、再解析後に再構築します。moduleのJSON metadataは別名や型の識別に使います。

ASTは式・引数・束縛名の元ソースのtoken範囲を持ちます。`symbols`は通常のcheckerと同じ規則で確認できた型を、元ファイルのUTF-16位置とともに返します。編集補助では失敗した文の変数環境を戻して次の文を解析しますが、通常の`check`は最初のエラーで失敗します。型エラーのあるコードから補完情報が得られても、ビルド可能になったことを意味しません。

定義ジャンプ用のローカル名は、型・所有権の検査とは別にASTをたどって解決します。引数・最初の代入・for・caseの束縛位置を持ち、再代入では同じ位置を保ちます。子ブロックの名前は外へ漏らさず、forの同名束縛はループ後に元へ戻します。使用位置と定義位置は既存の`references`へ出力します。move後や型の不明な初期化でも、名前の束縛先を特定できれば移動できます。VS Codeは未保存バッファも渡しますが、保存済み情報へフォールバックした場合にはF12でその位置を使いません。

codegenのscopeはTokioのJoinSet wrapperにします。classはRustのstructへ生成し、対応するフィールド型の場合にJSONやDB用の実装を付けます。現在のCommon IRはSSAでも独自optimizerでもありません。最適化はRust backendに依存します。

同じコンパイル処理の中では、loweringとcodegenが生成行と元の文・定義・フィールドの行の対応を保持します。ビルドではCargoのJSON診断を読み、対応するNagi・Lowのファイルと行を先に表示します。Rustの補足や修正候補は生成Rustの座標のまま残し、手書きRustや位置の不明な診断は書き換えません。

保存した`generated.low`を別のコマンドで読み直すと、診断位置はLowの行になります。moduleのIDは残りますが、元のHighへのsource mapを保存する機能はありません。

`check`はNagiの規則を検査します。生成Rustの借用・trait検査を代替しないため、成功後に`build`が失敗する場合があります。詳しくは[所有権](ownership.md#借用と検査の範囲)を参照してください。

ソースは1ファイル2 MBまでで、式・型・ブロックの入れ子にも上限があります。構文の変異試験と不正な入力の試験を行います。coverage-guided fuzz、incremental parsing、式の厳密な列位置や全Rust診断を扱うsource mapは未対応です。引用符なしの登録済み標準moduleは[利用できます](modules-and-rust.md)。

実装は[parser](../compiler/src/parser.rs)、[checker](../compiler/src/check.rs)、[codegen](../compiler/src/emit.rs)を参照してください。[所有権の境界テスト](../compiler/tests/ownership_boundaries.rs)では、Highと再解析したLowの検査、および一部の例の生成Rustの一致を確認します。[標準importのテスト](../compiler/tests/stdlib_imports.rs)では、保存Lowの定義IDを確認します。これらは全プログラムの等価性を保証するものではありません。
