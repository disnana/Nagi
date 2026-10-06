# Task完了・Nagi 0.1.11準備の根拠

S1 PR #88、agent構成 PR #89、S2 PR #90をmainへ反映した後のrelease準備記録。0.1.11の正式公開とversion PRの最終検証は、公開readbackを追記するまで未完了。現状は[release引継ぎ](../../../docs/internal/handoffs/2026-10-06-task-release-0.1.11.md)を読む。

- `s2-ci/`: head `f1497053`、checks run `37545273020`のLinux全suiteと4 OS package原ログ。`validated.json`は110契約、native7、runtime17、public3、doc9、両例三構文、抽出Task gateをログから独立確認した結果。`validate.py`はCargo stdout/stderrのheader順と次のrunning境界を分け、0件filterを成功群へ数えない。
- `s1-final-ci/`: head `08e90c6`のchecks `37540011861`に対する保存集計。過去の90契約headを今回の110契約実行として数えない。詳細source/測定/nativeログはS1 artifactにある。
- `s2-local/`: application全10 project/19実行とfuzz原ログ。1000 mutation、固定seed、bounded native16はcoverage-guided探索や任意schedulerの証明ではない。
- `published-0.1.10-audit/`: `nagi-v0.1.10`からS1最終sourceへのAPI差分、実Rust embedding smoke、version参照の分類と独立worker記録。これらは元の時点のsnapshotなので、当時のCI未完了表記を最新状態の根拠にしない。embedding smokeはRust文字列生成の検査で、生成Rustのnative実証とは別。
- `release-local/`: workspace/lock版0.1.11の`cargo check --locked`、release build、release手順98 unit、実0.1.11 CLIによるlibrary15検証/application10 project・19 native実行、website92頁が成功。unit内の模擬release表示はGitHub公開の証拠ではない。
- `provenance.json`: 実GitHub head/merge/tree/run、candidate source SHA-256、原artifact SHA-256と未検証範囲。この記録自身のcommit SHAを本文へ自己参照しない。

compiler/runtime本体のTask実装はS1から不変。S2は例・test・verifier・Docs・CIのみ、release工程はroot workspaceとlockの自package版のみ変更する。public API追加の再実装、SQLite公開化、一般Future、fault recoveryは混ぜない。cacheありの確認をclean buildと呼ばず、ignored費用群、infra失敗、0件、skipを意味論の成功へ加えない。
