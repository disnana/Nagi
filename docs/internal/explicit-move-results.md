# 明示moveの実装と検証

2026-10-06。移行前監査の基点はmain `e7aff1d`。明示moveの意味論は作者が承認済みで、[実装順](value-task-implementation-plan.md)のV1/V2を作業branchで実装した。moveの変更は未マージ・未リリース。#85は作者がマージ済み。#86はmain `f7799fa`との競合を解消し、更新head `93ad011`の必須CIとwebsite CIが成功した。その内容をmoveの独立draft PRへ取り込んだ。

## 実装の境界

`std.ownership.move`をcanonical標準operationとして解決し、High・保存Low・手書きLowで同じcheckを使う。操作は入力を一度評価し、checked planからRust標準の`std::convert::identity`へ値として渡す。新しいNagi runtime helperやcloneで移動を代用しない。既存非Copyの所有ローカルを右辺そのものにした代入は拒否し、新値の生成とCopy代入を受理する。引数・return・field/indexの既存consume規則は維持する。

[標準操作](../../compiler/src/stdlib.rs)、[checker](../../compiler/src/check.rs)、[封印plan](../../compiler/src/check/checked.rs)、[生成](../../compiler/src/emit.rs)、[定数評価](../../compiler/src/constant_eval.rs)が担当する。viewのorigin、async関数のprovenance、定数評価は同じwhole-value対応を利用する。値の転送と元placeそのものの借用は区別する。所有権・scope・cleanupのモデルは置き換えない。

## 先行失敗とレビュー

旧checkerが暗黙代入を受理する失敗と、標準moveが未登録の失敗を別に保存した。tests-only `dd044e1`の7群は旧compiler libraryに対して全て失敗した。単にparser拒否を期待した検査ではない。

全workspaceが一度通った後の独立レビューで、Copy集約値を比較する式にmoveを挟むとplace loanが消えるP1を発見した。`move(pair.number) == take(pair)`でOption fieldと非Copy親を同式で使う反例はcheckが通り、対応するRustはE0505になる。先行`ffd20f1`の追加群が失敗し、`d74e965`ではplace loanを透過する案を試した。続く検査で、括弧だけの生成では`len(move(values))`が元Vecを実際には移さず、Dropが遅れることも分かった。共通原因は、値を移す操作をRustのplace式のまま生成したことだった。

既存fixtureは所有ローカル代入だけをmoveへ移行した。importによる元行の移動、resolverのcanonical関数名、identity RHSの生成表現に内部期待を合わせた。借用拒否、Drop順序、source origin、flowの疎性・再checkの期待は維持した。生成Rustをpatchしていない。

## 検証状況

`dcd7cab`の全回帰は93 suite・892件成功。#85/#86を統合した`29a4618`（tree `9a23bf5cc4c55818fe3382c7007b7bb7c1e0167f`）では93 suite・902件成功、失敗・ignoredは0件。compilerとtestsのbytesは変えず、#85のprivate runtime修正を統合して確認した。4 OS CIは公開後に確認する。以前の7 suite・36件と正常18実行は[移行前監査](value-task-audit-results.md)で、新仕様の成功件数には加算しない。

| 検査 | 結果と範囲 |
|---|---|
| 明示move専用 | [explicit_moves](../../compiler/tests/explicit_moves.rs)の9群。正常3群はHigh・保存Low・手書きLowで計9 native build/run。裸の非Copy代入、move後使用、borrow/shared、temporary、branch/loop、Copy・新値、引数/return、定数評価、破棄/取消を検査 |
| 保存Low | 元Highを削除してから保存Lowを再checkし、直接生成と同じ出力・native assertionを確認。独立手書きLowも同じcheckerを使う |
| 基本品質 | fmt、clippy全target（warningsを拒否）、固定corpus42 sourceと17 harness登録、CI Python52件が成功 |
| 限定生成 | 18 grammarを使う256 case（seed 3735928559）。High/Low・rustc・native oracle成功。property全般や任意program生成の保証ではない |
| fuzz smoke | seed 305419896、10,000 text mutation。parse拒否7220、check拒否1796、受理後Low check/emit984、panic0。別に128限定生成をnativeまで実行。coverage-guidedではない |
| 日英Docs | 7箇所・4種類の完全例を現compilerでcheck/runし出力一致。website90ページ、変更文書の相対リンク/anchor639件、Python比較例12個の構文確認が成功 |
| 既存プロジェクト | 10プロジェクト・19実行（High/保存Low、手書きLow）成功。Axum native testは各source経路で8件、失敗・ignored0 |

原ログ・command・source hashと計測は[検証記録](../../benchmarks/results/explicit-move-2026-10-06/README.md)に保存した。チェックのwarm時間が旧compilerより長い観測も残している。Futureサイズ・allocation・速度の一般的な改善は主張しない。

並列installation fixtureで、コピー直後のcompiler起動が`ETXTBSY`になる失敗も記録した。同条件の小Rust試験ではcopy→execが800回中205回失敗し、読取り専用seedからのhard link→execは800回とも成功した。fixtureは一度だけseedをコピーし、個別配布先へhard linkする形に変更した。起動のretryやテスト全体の直列化は追加していない。seedの寿命・異なる配布先・最後の所有者による削除を1件追加し、既存3件と合わせて確認した。Linuxでの具体的なFD継承機構は未確認であり、観測した失敗から断定しない。

## 保証の範囲

対応した代入・move・借用・分岐の有限ケースを継続検査する。任意のNagiプログラムのcheck/build一致、全allocation、全Future配置、実行時I/O成功を保証する検査ではない。Task結果handle、業務Errとtask故障の新経路、公開SQLite Pool/Transactionは後工程。mergeとreleaseは別途判断する。

## 値として渡す実装の選択

| 生成案 | 判定 |
|---|---|
| operandを括弧で包むだけ | 不採用。借用先では元placeが残り、move済みとした所有値のDropが遅れる。Copy集約値の比較も元placeを借りる |
| blockでoperandを返す | 不採用。一時的なStringから借りたviewのownerがblock出口で消え、E0716になる |
| 単要素tupleからfieldを出す | 小Rust比較では動作したが不採用。値カテゴリとtemporaryの規則を手製の表現へ持ち込む必要がある |
| Rust標準`identity`へby-valueで渡す | 採用。Nagi checkerが型・consume・originを確定し、封印planが一度だけ渡す。Rustの通常の値引数・temporary lifetimeを使う |

[公式実装](https://doc.rust-lang.org/stable/src/core/convert/mod.rs.html)は`pub const fn identity<T>(x: T) -> T { x }`で、`#[inline(always)]`を持つ。全環境で命令数0になるとの保証にはしない。新しいNagi runtimeを増やさず、既存のby-value規則で実現する。比較、読み取りAPI、temporary view、RHS失敗、Drop/取消の実検査と、同条件のFuture計測で選択を確認する。

追加した比較群は「moveの結果は値」という契約から正例へ修正し、元のbare field/index比較での借用拒否は残す。この変更は新しい回帰群のoracleの誤りを正すもので、既存の拒否を受理へ緩める変更ではない。元の失敗と試行案も記録に残す。
