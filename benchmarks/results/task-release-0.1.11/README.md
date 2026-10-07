# Task完了・Nagi 0.1.11準備の根拠

S1 PR #88、agent構成 PR #89、S2 PR #90をmainへ反映した後のrelease準備記録。0.1.11の正式公開とversion PRの最終検証は、公開readbackを追記するまで未完了。現状は[release引継ぎ](../../../docs/internal/handoffs/2026-10-06-task-release-0.1.11.md)を読む。

- `s2-ci/`: head `f1497053`、checks run `37545273020`のLinux全suiteと4 OS package原ログ。`validated.json`は110契約、native7、runtime17、public3、doc9、両例三構文、抽出Task gateをログから独立確認した結果。`validate.py`はCargo stdout/stderrのheader順と次のrunning境界を分け、0件filterを成功群へ数えない。
- `s1-final-ci/`: head `08e90c6`のchecks `37540011861`に対する保存集計。過去の90契約headを今回の110契約実行として数えない。詳細source/測定/nativeログはS1 artifactにある。
- `s2-local/`: application全10 project/19実行とfuzz原ログ。1000 mutation、固定seed、bounded native16はcoverage-guided探索や任意schedulerの証明ではない。
- `published-0.1.10-audit/`: `nagi-v0.1.10`からS1最終sourceへのAPI差分、実Rust embedding smoke、version参照の分類と独立worker記録。これらは元の時点のsnapshotなので、当時のCI未完了表記を最新状態の根拠にしない。embedding smokeはRust文字列生成の検査で、生成Rustのnative実証とは別。
- `release-local/`: workspace/lock版0.1.11の`cargo check --locked`、release build、release手順98 unit、実0.1.11 CLIによるlibrary15検証/application10 project・19 native実行、website92頁が成功。unit内の模擬release表示はGitHub公開の証拠ではない。
- `release-local-after-fix/`: #92 main統合後の実0.1.11 release build/148契約、task-resultsとsupervised-serviceのHigh/元High削除後の保存Low/独立手書きLow、計6 native実行が成功。旧headのlocal検査と区別する。版PRの新しい最終headの全回帰/4 OS・正式公開はまだ別gate。
- `conditional-fix-final/`: release前レビューが見つけた短絡/lazy式のTask義務の修正。独立post-fixレビューは旧6再現のchecker/元行拒否、独立checker58件、専用三構文native21経路、loop/user envの追加native3実行を確認し未解決0。rootのnetwork付きworkspace95 result block/936成功/failed0/費用ignored1は別実行の原ログとして保存した。`ci/`の原ログ/validatorは最終head `8db1607`の4 OS各148契約/Task native8/runtime17/public3/docs9/両例三構文/展開archive Taskを確認し、Linux全936成功/fuzzも保存。main `97e62f82`へのmergeと同treeを読み戻した。修正を統合した0.1.11最終CI・正式公開は別gateで、このfix完了を代用しない。
- `provenance.json`: 実GitHub head/merge/tree/run、candidate source SHA-256、原artifact SHA-256と未検証範囲。この記録自身のcommit SHAを本文へ自己参照しない。

S2は例・test・verifier・Docs・CIのみ。release前に別fix PR #92でcheckerの条件付き消費を修正し、runtime/emitter/API/依存は維持する。release工程の版差分はroot workspaceとlockの自packageだけ。public API追加の再実装、SQLite公開化、一般Future、fault recoveryは混ぜない。cacheありの確認をclean buildと呼ばず、ignored費用群、infra失敗、0件、skipを意味論の成功へ加えない。
