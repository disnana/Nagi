# ADR 006: 最終checkとRust生成の間を封印する

状態: Phase 1の採用設計。実装・検証結果は[進捗](../progress.md)へ記録する。

## 問題

従来の`emit::rust(&Program)`は、parserの出力、編集途中のAST、最終check後のASTを型で区別できなかった。生成側もCopy・resource capability・view storage・引数の渡し方を再判定していた。checkの判断と生成時の判断が分かれる入口である。

`Program`自体を全面的なTyped IRへ置き換えず、最終Low/native統合後だけを明確な境界にする。

## 採用

最終factoryはprimary Low AST、native Low AST、source provenanceを受け取る。native統合・`@replace`検査、通常check、定数・route条件、checked factsの整合性、生成planの確定が成功した場合だけ`CheckedProgram`を返す。

`CheckedProgram`はProgramを所有し、外部から構築・可変化・取り出しできない。閲覧は共有参照に限る。初回High checkやエディターの回復ASTからRust生成へ直接進む入口は残さない。

任意のschemaを検査したという証明は含めない。`--sql-schema`の検査はCLIが封印後のcanonical ASTを読み、Rust生成前に行う。schemaを指定しない通常checkにSQL検査を追加しない。

[ADR 011](011-language-behavior-and-docs.md)の方針でもこの境界を維持する。CheckedProgramは検査済みNagiとRust生成向け確定情報を渡すもので、完全なbackend非依存IRでもruntimeの終了完了の証明でもない。別backend、self-hosting、Low構文の互換性は別の判断である。

封印時に従来の判定関数と順序を使い、Rust名へ変換した私有AST、derive・resource access・呼出しpassing・view storage・cleanup・statement出力の決定を持つ。canonical AST/definition IDとRust上の綴りは別に保持する。emitterは確定planを印字し、capabilityやmove/borrowを独自に再推論しない。

## 保持するもの

- High→型付きLow text→Low再parse/check。保存Lowも独立入力として扱う。
- Low構文、`@replace`、公開Type、既存の受理・拒否、評価順、Dropと取消の意味。
- Rustのtrait、外部API、Send/Sync、native本体、最終borrow安全性の検査。
- 現在の文・定義行単位のsource mapping。式columnやfull spanは推測しない。

生成由来と元位置は別の情報にする。同一コンパイルのGenerated Low、独立入力のUser Low、Native Low、置換本体と対象definition、合成glueを区別する。Rust native本体の位置を近いHigh行へ割り当てない。

## 不採用

- Programを包むだけで生成側の再推論を残す。入力API以外の問題が残る。
- finalizerでRust textを生成して保存する。checked representationとbackendの役割が混ざる。
- text往復の撤去、全面HIR/SSA、独自Rust borrow checker。今回の境界整理には不要で、互換性の確認範囲が広がる。
- facts欠落をdefault、暗黙clone、再checkで埋める。内部不整合の観測を隠す。

## 検証の境界

未検査Programのemit、外部構築、mutable accessはRust APIのcompile-failで確認する。意図的に壊したchecked factsは再checkせず封印の拒否を確認する。通常の既存fixtureはfinal factoryを通し、元のassert・High/Low比較・native実行・縮小oracleを保つ。

同じ入力のcanonical facts、plan、生成Low/Rustの決定性、実ファイルのprovenance、既存conformance・Drop・Scope取消、bounded生成とmutationを確認する。旧版と生成内容・frontend時間も比較する。全プログラムの一致や全Rust diagnosticの分類を証明したとは扱わない。

出典: [段階計画](../compiler-rust-boundary-plan.md)、[ADR 004](004-checked-boundaries.md)、[内部pipeline](../compiler-pipeline.md)。
