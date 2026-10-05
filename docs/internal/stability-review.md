# 処理系の安定化レビュー

調査基点: `5c348fd`。Nagi 0.1.10・VS Code 0.1.13の版番号は変更しない。ここに記す修正はUnreleasedであり、既存の配布版へ入ったことを示さない。

## 調査と適用

一次資料と採否は[参考メモ](compiler-testing-research.md)にまとめた。Codex/AGENTS.mdからは短いroot指示と用途の分離、rustc/LLVM/Clang/Swiftからは段階別の検査、型検査済みoperandとcleanup位置、source identityと合成位置の区別を採用した。Goの評価順・temporary、Proptestの生成・縮小・失敗保存、Csmith/Craterの限定oracleも参考にした。

NagiにMIR/SSA、ARC/GC、独自drop flagsを移植しない。Rustのborrow checker、Drop、Future変換を使う。TypeScriptの公式実装元の解説は責任分離の参考にし、Nagiの規範的仕様にはしない。Kotlin/Zigを調査したとは主張しない。

README/DESIGN/CONTRIBUTING、公開Docs、`ai/`、compiler/parser/checker/emitter/SQL、runtimeのHTTP/DB/Scope/actor、既存テスト、CI・release scripts、実アプリを照合した。実装済み、対象ケースで検証済み、制約、未採用の構想を分けた。独立VM・machine-code backend、Lowのpointer/layout/unsafe/C ABI/SIMD、PostgreSQL・transaction/poolの標準APIは今回追加しない。

## 設計上の問題と共通原因

1. **借用元と格納先の寿命が結び付いていた。** 返却まで使う一つのRust変数へ局所viewと入力viewを順に代入すると、Nagiが受理した短い借用がRustでは長い寿命へ拘束された。List専用の退役値ではResult/Optionへ広げられなかった。
2. **checkerとemitterが使い方を別々に決めていた。** 名前や型からmove/borrowを再推測する方式は、ユーザー関数・別名・nested wrapperへ広げるほどずれる。loop初回のtyped bodyと固定点後の状態の混在も同じ問題だった。
3. **生成がソースにない境界を作っていた。** Scopeのinner asyncは局所借用を別coroutineへ逃がし、env fallbackのclosureはsourceの`try`/`await`を別の出口へ入れていた。返却viewを持つ関数だけのScope修正では、一時的にviewを読む関数にも問題が残った。
4. **検査の観測点が揃っていなかった。** checker成功だけのテストでは生成Rustの拒否を検出できない。縮小器にも、oracleの参照先を削除して別のE0425へ縮む問題があった。

High→Low text→再checkは現状維持する。Low textは通常CLIの内部境界でもあり、単なるdumpではない。一方、Lowの最終check後に所有権の意味を再推測する必要はない。[経路の比較と移行条件](compiler-pipeline.md)に、共通CheckedProgram案、source/module/@replaceの移行、費用測定、旧経路との比較を残した。全面rewriteは行わない。

## 不具合の分類

| 優先度 | 問題 | 対応・根拠 |
|---|---|---|
| P0 | 今回の対象から新しいunsoundness・data corruptionは確認していない | 全リポジトリの安全性証明や独立security auditを行ったという意味ではない |
| P1・修正 | Result/Option storage、candidate依存for/while、asyncのcheck/build不一致 | `view_flow_completion.rs`、`view_container_drop.rs`でHigh/保存Low/手書きLowを実Rustへ通す |
| P1・修正 | planned mutable receiverの先行borrowによるE0502 | `append(parts, parts[0])`をitem評価→mutable projectionにし、RHS panicも確認 |
| P1・修正 | Scopeの余計なasync境界によるE0521/E0381 | 全Scopeを同じlexical/error exitモデルへ統一。借用返却とscalar返却、await、nested error、親取消、body panicを実runtimeで検査 |
| P1・修正 | env fallback内のtry/awaitが生成closureから出られない | lazy fallbackをmatchに生成。存在時の非評価、missing時のErr/awaitを同じ3 source formsで検査 |
| P1・未解決 | `1 / (1 - 1)`等の定数式はcheck成功後にRustで拒否される | 下記の再現・必要な判断・次の行動を参照。crate/trait委譲ではない |
| P2・修正 | backend縮小がoracle由来E0425へすり替わる | oracle無しの生成Rustでも同stage/error codeになる場合だけ縮小。自己テストを追加 |
| P2・継続 | text再解析でchecked factsを失う、行のみのsource map、ASTにoptional型情報 | 現経路と制約を文書化。今回のprivate planは全面Typed IRではない |
| P2・継続 | owning-view metadataにList由来の名前が残る | 私有名なので互換性問題ではない。次の関連変更で整理し、公開型変更と混ぜない |
| P3・継続 | private storage/Future frameの費用、探索のcoverage拡大 | payload cloneで回避しない。以下の条件付き測定と定期探索を継続 |

### 未解決P1: 定数算術

```python
def main():
    print(1 / (1 - 1))
```

実CLIの`check`はexit 0、`build`はexit 1。Rustの`unconditional_panic`で拒否され、primary位置はNagiの2行目へ戻る。既存のliteral 0診断と、定数式・overflowの解析は別である。Rustのlintを無効化して成功にする修正は採らない。

今回ここを直していない理由: 現行の採用済み算術規則はliteralの0を対象とし、定数式・推論・各整数幅・overflowまでの診断優先順位が未確定。借用生成の修正へ別の定数評価モデルを混ぜず、P1として独立に追う。

次の行動: 副作用のない定数整数式について、型検査後の評価範囲、overflow/zeroの区分と位置、Rustとの受理差を先に定義する。8整数型×High/保存Low/手書きLowのpass/failを追加し、共通評価モデルで診断する。実行時のゼロ除算をpanicからResultへ変更する案とは分ける。

## 実装の変更

- checkerがBindingId付き`Move/Copy/Borrow/BorrowMut`を記録し、生成planが使う。loopは最終固定点の条件/bodyを保持する。欠落factsをbuiltinの推測で隠さない。
- 返却につながるowning view containerへ私有`Option<T>` storageを使う。公開型は変えず、RHSを評価して新値を元cleanup anchorへ置き、旧値を退役させる。
- 全Scopeを同じcoroutine内のlabelled Result blockへ生成する。body localsを片付け、最寄りScopeで取消を待って外側へErrを伝える。通常Try、Rust Error変換、body後のjoinを維持する。
- envのfallbackをclosureへ入れず、成功値/失敗時のfallbackをmatchで生成する。fallbackの遅延評価と既存の所有文字列への変換を維持する。
- runtime交換、新しいsyntax、Low削除、新dependency、生成Rustの直接patchは行わない。所有値の暗黙cloneも追加しない。

## 文書・エージェント環境

root [AGENTS.md](../../AGENTS.md)はリポジトリ開発用、`ai/`はNagiアプリ作者用。[言語契約](language-invariants.md)、[テスト分類](compiler-testing.md)、[生成経路](compiler-pipeline.md)、[参考資料](compiler-testing-research.md)を判断の根拠にする。

DESIGN/README/CONTRIBUTINGの日英、公開ownership/compiler-internals/roadmap、AI向けlanguageも実装に合わせた。HTTPのpanic捕捉はrollbackではない、取消は受理済みDB処理を戻さない、静的SQL検査はopt-inで値型/NULLまで保証しないことを残す。LowやRust資産との連携を、独立VM・生ポインター・無制限なRust型公開と混同しない。

## 検証

Linux x86_64 / rustc 1.98.1で次を確認した。

| 検査 | 結果 |
|---|---|
| `cargo fmt --all -- --check` / workspace全target Clippy | 成功 |
| `cargo test --locked` | exit 0。登録734テスト、失敗・ignoreなし。native子プロセスのテストはこの件数へ重複加算しない |
| 追加conformance harness | 5テスト: corpus/生成、frontend縮小、oracle由来errorの誤縮小防止、Rust文字列等を保つbatch隔離、process期限/回収 |
| 追加storage/Scope生成harness | 3テスト。22個の小関数と0/1/3/10回・分岐をHigh/保存Low/手書きLowへ通す |
| 既存Drop/Scope integrationの拡張 | RHS/旧Drop panic、Result/Option、async取消、nested Scope/body/join Err、借用のscalar返却、env fallbackを実行 |
| 2 seedのconformance拡大 | 各256生成＋31 corpus、各5 harnessテスト成功 |
| 2 seedのfuzz拡大 | 各10,000 mutation＋128 bounded native source case、panic 0 |
| 9実アプリ | High/保存Low/手書きLowの17経路、check/build/run成功。Axum連携・HTTP・JSON・SQLite・Supervisorを含む |
| CI script unit / workflow lint | 51テスト、actionlint成功 |
| 文書・サイト | Markdown167ファイル/ローカルリンク1,286件、サイト90ページ/リンク・asset9,622件、エラーなし。外部105リンクの到達確認は別 |

seedは305419896/3735928559。mutationのparse拒否は7,149/7,087、check拒否は1,910/1,904、受理後Low再check/emitは941/1,009。通常の拒否をpanicとして数えず、受理だけをnative成功とも数えない。新commitのWindows/macOS実行結果はPR CIのreadbackで別途確認する。

31件の外部corpusは15 run-pass、16 compile-fail。各negativeは拒否stage・意味・行位置を固定する。過去のshared field move、owned Result discard、static/function-value view、sequential lifetimeをpass/failで残した。HTTP panic、SQL列、Scope、Drop等は9つの既存実harnessに接続し、runtime stubを保証に使わない。

標準の生成は10種のbounded grammarを24件。High parse/check→Low text→Low parse/check→直接/保存Low Rust→実rustc→native oracleを通す。これは共通backendを持つ限定的な観測同値検査で、一般的なNagi対Rustのdifferential testingではない。

失敗はseed・source・期待stage・診断・縮小sourceとcaseを保存する。UTF-8/stage/signatureを守り、予算とprocess deadlineを持つ。定数値を縮めたruntime caseはhost oracleも再計算し、固定corpusを意味の変わる削除で縮めない。縮小は数学的な最小性を保証しない。

### 費用

Linux x86_64、rustc 1.98.1、今回のasync resource fixtureでは生成Future 168 byte、同じ引数/資源/await/返却でcaller由来の借用だけを使う比較例136 byte。32 byte増えた。High/保存Low/手書きLowが同じ値であり、optimized native testでも168/136だった。ABI、他target、他のprogramで同じサイズになる保証はない。

resource fixtureはbufferの確保・解放、element Drop、RHS Err/panic、旧値Drop panic、Future未poll/pending/再開/Err/panic/取消を観測する。追加のpayload clone/allocationなしを対象ケースで検査する。これを全ランタイムの無allocation、全コードの速度改善とは説明しない。

## fuzz / propertyとCI

Proptestは調査したが今回新crateを加えない。小さいgrammarと既存stdでseed生成・縮小・保存を実装した。strategyが増えて構造的shrinkingの維持費が高くなったらdev-dependencyを再検討する。

fuzz smokeは任意text mutationのparse/checkを通し、accept後はLow再解析・再check・Rust emitを要求する。native検査は狭い生成caseだけで行う。parser panicだけを数えずstageごとにaccepted/rejectedを分ける。coverage-guided fuzz、sanitizer、任意文字列への大量Cargo投入は今回行わない。

- PR/push: 既存の変更検出、fmt/clippy、全Cargo、31 corpus＋24生成、fuzz smoke、既存HTTP/SQL/actor/サンプルを維持。4配布targetの検査にconformance/storage/Scopeを追加。Docs/AGENTSだけならRust全検査を省く。
- 定期: 毎週月曜03:17 UTC/手動。2固定seedそれぞれ256生成、10,000 mutation、128 native、重要view/Scope/Drop。各job 30分上限。
- 失敗: `build/compiler-failures/`をartifactへ保存する。CIを追加したことと実際に対象commitで成功したことは分けて確認する。

## 現在の保証範囲

維持する責任は、対応するNagiの型・move・view・エラー規則を検査し、受理した意味を生成Rustへ保つこと。今回の回帰例とbounded生成を継続検証する。Rustへ任せるcrate API/traits、最終Send/Sync/Clone、依存・link・target環境とは分ける。

全受理プログラムのcheck→build成功、全ownership組合せ、完全な診断source map、未知のICE不存在、unsafe Rustの安全性、panic後の状態rollback、プロセス障害回復、任意プログラムの隔離は保証しない。定数算術の既知P1も残る。CI成功やテスト件数を言語全体の正しさの証明にしない。

## 次の3項目

1. 定数算術の既知P1を、型幅・overflow・診断順序を含む共通モデルとconformanceで閉じる。
2. 定期探索で得た反例を恒久corpusへ移す。generated grammarへfunction value、複数origin、Scope/error boundaryを段階的に追加する。
3. source/module identityとchecked factsのtransportを測定し、必要ならCheckedProgram境界へ移行する。Lowの公開互換性を保ち、Rustへの委譲を増やせる箇所は任せる。
