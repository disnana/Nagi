# コンパイラと開発エージェントの参考資料

調査日: 2026-10-05。実装の調査基点は `5c348fd`。以下は参考資料とNagiでの判断であり、他言語の仕様をNagiの仕様として採用するものではない。

| 一次資料 | Nagiに関係する点 | 採用する考え | 採用しない・保留するもの |
|---|---|---|---|
| [Codex: AGENTS.md](https://developers.openai.com/codex/guides/agents-md)、[AGENTS.mdの形式](https://agents.md/) | 作業ディレクトリに応じて指示を読み、近いディレクトリの指示で補足する。Codexの既定の合計読み込み上限は32 KiB | rootには実行できるコマンドと判断規則を短く置き、契約や背景は内部文書へリンクする。アプリ作者向けの`ai/`と処理系開発者向けの`AGENTS.md`を分ける | 一般的な心構えの長い列挙、同じ規則のディレクトリごとのコピー。指示ファイルだけで遵守を保証する考え |
| [rustcのテスト基盤](https://rustc-dev-guide.rust-lang.org/tests/intro.html)、[UI tests](https://rustc-dev-guide.rust-lang.org/tests/ui.html)、[追加方法](https://rustc-dev-guide.rust-lang.org/tests/adding.html) | 受理、診断、ビルド、実行は別の観測。check-passとrun-passは同じ保証ではない | 小さいソースを保存し、通過すべき段階・拒否する段階・診断位置・実行結果を記録する。pass/failを対にする | rustc用compiletestの直接導入、環境依存のRust stderr全体を固定すること |
| [Proptest](https://proptest-rs.github.io/proptest/intro.html)、[開始方法](https://proptest-rs.github.io/proptest/proptest/getting-started.html) | strategyで入力を生成し、shrinkingと失敗の保存で再現を残す | まず既知の借用・Result・分岐を狭く生成し、失敗段階を保つ縮小を行う | 今回は新crateを追加しない。小さい生成器で十分か測定し、組合せが増えて独自shrinkerの維持費が上がった時に再検討する |
| [Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz.html)、[LLVM LibFuzzer](https://llvm.org/docs/LibFuzzer.html) | 決定的なtarget、seed corpus、crash保存、縮小。coverage-guided探索には専用基盤が必要 | 既存smokeを段階別にする。拒否は正常、panicや受理後の失敗は異常として区別する | stableの固定変異試験をcoverage-guided fuzzと呼ばない。任意文字列すべてをCargoへ投入しない。cargo-fuzz/nightly/sanitizerは別laneとして保留 |
| [Csmith](https://embed.cs.utah.edu/csmith/)、[Crater](https://rustc-dev-guide.rust-lang.org/tests/crater.html) | 意味の定まる入力と独立oracle、実プロジェクトでの回帰確認 | Nagi checkerと生成Rustの受理結果を限定比較する。High/Lowの実行比較には独立した期待値も使う。PRと定期実行で件数を分ける | Rustを一般的なNagiの仕様oracleにしない。共通backendを使う二経路の一致だけで正しさを主張しない |
| [THIR](https://rustc-dev-guide.rust-lang.org/thir.html)、[MIR construction](https://rustc-dev-guide.rust-lang.org/mir/construction.html)、[borrow check](https://rustc-dev-guide.rust-lang.org/borrow-check.html)、[drop elaboration](https://rustc-dev-guide.rust-lang.org/mir/drop-elaboration.html) | 型検査後のplace/operandと破棄scopeを明示する。再検証にも目的がある | 値のmove・copy・borrow・変更をchecker側で決め、生成側へ渡す。Rustのborrow checkとdrop/unwindを使う | MIR、独自drop flags、独自GCをNagiへ移植しない。「lowering後には何も検査しない」と解釈しない |
| [rustc AST lowering](https://rustc-dev-guide.rust-lang.org/hir/lowering.html)、[SourceMapの実装](https://github.com/rust-lang/rust/blob/HEAD/compiler/rustc_span/src/source_map.rs) | 元のnodeと合成nodeのID、ソース位置を分ける | 名前の表記、束縛ID、生成slot ID、診断元位置を混同しない | 今回は全ASTのID体系やsource mapを全面交換しない。保存LowにHighのsource mapが残るという説明はしない |
| [Swift SIL](https://github.com/swiftlang/swift/blob/main/docs/SIL/SIL.md)、[Ownership SSA](https://github.com/swiftlang/swift/blob/main/docs/SIL/Ownership.md)、[testing](https://github.com/swiftlang/swift/blob/main/docs/Testing.md) | 型付きIRの段階、consuming/nonconsuming operand、verifier、stress testの分離 | 公開のNagi型と私有の生成storageを分ける。生成planの前提を検証する | OSSA、ARC、全CFGの移植。Rustの版やtargetで変わるFutureサイズの固定値を共通CIへ置かない |
| [Clang Internals](https://clang.llvm.org/docs/InternalsManual.html)、[LLVM TestingGuide](https://llvm.org/docs/TestingGuide.html) | ソースに忠実なASTとsemantic情報、診断・IR・実行の検査を分ける | 元ASTをRustの名前文字列に書き換えて意味を埋め込まない。意味のある出力と位置を検査する | Nagiにないmacro/includeやLLVM backendの仕組みを追加しない |
| [Go compiler](https://github.com/golang/go/blob/master/src/cmd/compile/README.md) | syntax、型付き表現、walk、SSA。walkで評価順とtemporaryを明示する | 右辺評価、新値の設置、旧値の退役を一つの契約にする | GC、SSA optimizer、escape analysisを持つ新処理系へ広げない |
| [TypeScript compiler notes](https://github.com/microsoft/TypeScript-Compiler-Notes)、[emitter](https://github.com/microsoft/TypeScript-Compiler-Notes/blob/main/codebase/src/compiler/emitter.md) | checkerの情報とprinter/source-mapの責任を分ける | 名前解決・型情報を生成側で再推測しない | この解説だけを言語契約の根拠にしない。公式実装元の資料でも規範的仕様とは区別する |

KotlinとZigは今回の変更を決める根拠として調査していない。上の資料で必要な比較ができたため、参照したことにはしない。

## 今回の判断

HighをLowへ保存して再検査する経路は現行の契約として維持する。一方、Lowの再検査**後**にmove/borrowの意味を生成側で再構築する必要はない。checkerの事実と私有の生成planを使い、型に応じた特別扱いの追加を減らす。

テストは既存の実HTTP・SQL・scope・actor harnessを残し、共通の小さいconformance corpusを追加する。corpusに障害名を載せただけでは、その障害を検証したことにならない。

調査に基づく構成比較は[compiler-pipeline.md](compiler-pipeline.md)、実行方法と保証範囲は[compiler-testing.md](compiler-testing.md)にまとめる。
