# SQLite公開APIと導入Docsの引継ぎ (2026-10-08)

## 完成したsourceとPR構成

開始時mainは`676576724829e45b077b58628bfe2417e6cf3673`。Task/moveは0.1.11、JetBrainsは0.1.1で公開済み。今回のSQLite APIは未リリースで、merge/release/version bumpをしていない。

- [PR #98](https://github.com/disnana/Nagi/pull/98): 初アプリ・本体貢献の日英Docs、言語機能索引と不足説明、実行例。head `bc6a76bf2422aadaca799b739ab75e5f1e6bc450`の4 OS/Linux/IDE/package/website成功後、作業中に外部でmainへmergeされた。merge commitは`2d87d8354808b221d537a8b1555ca39d2b95ef89`、treeは検証済みheadと同じ`4d72a7d39ccd164400ce6a0e1353437b637f0ce7`。このagentはmerge操作をしていない。
- [PR #99](https://github.com/disnana/Nagi/pull/99): SQLite Pool/Transaction公開API・compiler・runtime・日英reference・検証。#98上の依存PRとして競合を解消後、#98のmergeを読み戻してbaseをmainへ変更した。旧baseはmainの祖先でtreeも同じため、再rebaseやproduction変更は不要。最終head/CI/ready状態はPR本文とChecksを参照する。

rebase前の検証sourceは`e3e0ea3962bd847a9ffdaef4fdabb7598844459f`。統合source `fbfebd3`とのcompiler/runtime/Cargo.lock/SQLite配布gate/費用harnessの差分は0。PR履歴のcommit SHAが変わっても[provenance](../../../benchmarks/results/sqlite-public-2026-10-08/provenance.json)のfile hashesで照合できる。

## 重要な設計判断

ユーザー承認に従いvendor限定案も代替依存も比較し、既存Tokio FIFO semaphoreとlazy専用adapterを採用した。deadpool/deadpool-runtimeを除去し、新依存・Tokio/rusqliteの版更新はない。Optionsはscalar/native範囲を検査し、全capacityの予約はしない。openはpath/policyを検査し、native workerはbeginでlazy起動する。実増分のfallible reservation失敗だけをALLOCATIONとして扱い、任意OOM/allocator abortの回復は保証しない。

8 type/resources、18 operationsをcheckerのcanonical metadataからsealed plan、Low/Rust生成、public runtimeへ接続した。旧db_*とTask/spawnは互換性を維持する。Txはsame-task affine resource、SQLはworker転送前に所有化、Parametersは型別owned builder。取消要求と終了確認を区別し、close成功はnative close＋actual joinまで待つ。acquire予算はlogicalからnativeへ同じ残予算を渡し、0ms immediate取得を許し、BEGIN/busy/SQLまで延長しない。

詳細は[確定判断](../sqlite-public-runtime-decision.md)、[ADR 010](../adr/010-sqlite-transaction-boundary.md)、[日英public reference](../../sqlite-pool.md) / [English](../../en/sqlite-pool.md)。過去のdeadpool比較・private bridge資料は履歴として保持し、現実装の状態と混同しない。

## 検証結果と証拠

[段階別results](../sqlite-public-results.md)と[保存artifact](../../../benchmarks/results/sqlite-public-2026-10-08/README.md)に原ログとhashを保存した。46/46契約入力（14受理、32checker拒否）、7正例×High/保存Low/手書きLowのnative、negative元位置、SQLite native 73、public API consumer1を確認。最終workspaceではsqlite_public8/8、全回帰exit0（raw文字集計970pass、0failed、1既存ignored、子process重複あり）、Task148/148、fmt/clippy、seeded fuzz、examples10projects/19runsが成功した。

SQLite教程High/独立保存Low、local linux-x86_64 archiveの展開compiler/runtime実行を確認した。公式配布物の更新はしていない。統合後はwebsite98pagesのlinks/anchors/assets、初アプリ10native、SQLite日英code一致、CI policy59 testsが成功。4 OSの最終結果はPR #99の固定headと各jobログを読み戻してPR本文へ記録する。source記録を作る時点のpendingを最終CI成功の証拠にしない。

独立runtime reviewはP0/P1/P2なし。独立compiler reviewのSQL一時loan過剰保持P2と誤ったログ参照P3は修正・追跡確認済み。Docsの初心者視点レビューとSQLite日英説明の不足も修正・再確認済み。費用測定は同runtimeの生成/手書きRustでFuture、calling-thread allocation、時間、binary bytesを記録した。全heap、他OS費用、clean-build費用、全interleavingは未測定。巨大allocationや資源枯渇実験は行っていない。

## 後続の順番と範囲外

1. PR #99の最終本文・Checksを読む。この依頼の完了点はPR作成と検証で、merge/release承認ではない。#98は上記の通り外部でmerge済み。
2. #99はmain向けに変更済み。後日mergeが指示された場合は、その時点のmain/head/必須CIを読み戻す。mainがさらに進んだ場合にだけ必要な統合と再検証を行う。
3. リリースは別指示でversion/changelog/互換性と配布gateを確認する。Nagi compilerのRust埋込み利用でmetadata enumをexhaustive matchする場合は新variant対応が必要。旧Nagi Db APIは維持する。
4. 任意Rust resource/一般effect・region、pool resize/min-idle/expiry、自動retry、他DB対応、全OOM回復、compiler finalization追加passのpeak memory最適化は今回の公開sliceに含めない。必要なら別PRと契約・oracleを用意する。

Docsの入口は[最初のアプリ](../../first-app.md)、[初めての貢献](../../contributing.md)、[言語機能索引](../../README.md)。全既存snippet、外部URL、GUI/IDE操作、各OSの手動インストールまでは実行していない。CLIの主要手順は自動native harnessと独立確認の範囲を明記した。

## 4 OSの固定source検証

head `e86a9419fbcc547e937a13f0c0bf3927f98053b8`の[checks 37706126168](https://github.com/disnana/Nagi/actions/runs/37706126168)はattempt 2で成功、websiteとpush checksも成功。Linux全回帰、Ubuntu/Windows/macOS arm64/macOS Intel package、IDE、merge gateを読み戻した。各OSでSQLite 46/46、sqlite_public 8/8、runtime 73/73、Task 148/148、初アプリ10native、SQLite教程と展開配布物のnativeを確認した。[CI証拠](../../../benchmarks/results/sqlite-public-2026-10-08/ci-readback.md)に実行job・原ログ・初回失敗・再実行範囲を保存した。

macOS Intelの初回はeditorのHigh/Low symbol取得2件が既存5秒期限でSIGTERMとなった。同一head・期限・並列設定の一度の再実行では201/201成功。原因は未特定であり、再実行を原因解決や安定性の証明としない。通常fixtureのLinux逐次比較ではsymbol取得が約0.43–0.46秒から約0.56–0.60秒へ増加した。後続候補はsymbol/catalog経路のprofilingとCI時間変動の調査であり、任意のtimeout拡張や公開SQLite契約変更はしていない。

後続のd723961も全4 OS/全回帰/websiteに一度で成功した。main保護ルールが最新mainの履歴を要求したため、作業branchへ履歴だけを統合したb4c8f83はd723961と同じtreeだった。公開PR/mainへのmergeは行っていない。

そのpush CIで既存Actor回帰の同期不足を観測し、旧世代のmailbox閉鎖を待ってから既存ready/assertへ進むtest-only修正を行った。[RED・独立source review・修正・local検証](../../../benchmarks/results/sqlite-public-2026-10-08/actor-ci-sync.md)を参照。元assert・期限・並列設定、Actor/SQLiteのproduction契約は維持した。以降の最終PR headのCIは本文とChecksへ記録し、過去headの成功と区別する。これを新しいActor仕様や一般的なready後のcall成功保証と解釈しない。
