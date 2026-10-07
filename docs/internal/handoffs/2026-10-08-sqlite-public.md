# SQLite公開APIの引継ぎ (2026-10-08)

## source・PRの状態

- base: main 676576724829e45b077b58628bfe2417e6cf3673
- source head: e3e0ea3962bd847a9ffdaef4fdabb7598844459f、tree e6d986d13345743465b74dfeb34f005a66a4c924
- 主な段階: runtime e78f35a、compiler 0f9dd79、SQL alias fix 0bebcd0、Docs/CI e3e0ea3。source branchは計5 commits。
- [PR #99](https://github.com/disnana/Nagi/pull/99)はmain baseのdraft。e3e0ea3のinitial checksは実行中で、latest-head CIの最終readbackはまだない。merge/release/version bumpなし。APIは0.1.11未収録。
- 導入Docsの[PR #98](https://github.com/disnana/Nagi/pull/98)は別branch、head bc6a76b。[checks 37701060949](https://github.com/disnana/Nagi/actions/runs/37701060949)と[website 37701060552](https://github.com/disnana/Nagi/actions/runs/37701060552)を確認し、4 OS/Linux/IDE/package/gate/website成功後ready、未merge。各OS raw jobのonboarding harnessは10 native cases success。PR #98結果はPR #99 CIの代用にしない。

## 変更内容

Q002/Q004承認に基づくSQLite-specific Pool/Tx API、compiler resource/ownership/SQL checks、日英referenceと既存SQLite/SQL Docs更新、website nav、example、CI、DESIGN、CHANGELOGをPR #99へまとめた。runtimeは既存Tokio FIFO semaphoreとlazy adapterを使い、deadpool/deadpool-runtimeを除去。新依存やTokio/rusqliteの版更新はない。public APIは8 types/resources、18 operations。既存db_* APIは維持され、SQLite新APIは未リリースのまま。

今回のcapacity承認で、巨大scalar設定のALLOCATIONはfallible reservationだけを示し、universal OOM回復保証にはしない。新APIのtransaction, close, failure, logical FIFO, SQL prepare-only limitsは[日英public reference](../../sqlite-pool.md)と[英語reference](../../en/sqlite-pool.md)を正本とする。古いdeadpool比較記録は履歴として保持し、現在のruntime dependencyと読み替えない。

## 主要な検証とそのstage

再利用cache上のruntime結果はe78f35a stage、compiler focused結果は0f9dd79 stage、同じowned SQL文字列を後続Parametersへmoveする修正と回帰は0bebcd0 stage、Docs code/website/schema preflightはe3e0ea3 stageで記録した。source revisionごとのログ・件数・範囲・hashは[検証結果](../sqlite-public-results.md)と[artifact README](../../../benchmarks/results/sqlite-public-2026-10-08/README.md)を読む。最新Linux workspace test/fmt/clippy、release build、examples、seeded fuzz、SQLite native sample、extracted local archiveは成功した。workspace raw logはgzipで保持し、95 result blocks/970 passedという文字集計にはchild processの重複を含むためunique test数として扱わない。SQLite runtime full suiteには73件のsqlite:: testsがある。別のfocused oracle logにある71件は後から加わった2件を含まない古いsnapshotである。

独立runtime reviewはP0/P1/P2 blockerなしで、logical semaphore waitに限るFIFO、retired=falseの非保証、ReplyLost/UNKNOWNの不確実性をDocsが扱うべきと指摘した。独立compiler reviewは一時SQL loanが後続Parameters moveを不当に拒むP2を発見し、argument 1 materializationだけでloanを解放する修正を追跡し、3 operation regression/native確認とTx-loan negativeをレビューした。独立Docs reviewのP2も修正済み。各reviewの限界は原記録に残す。

PR #98のready状態と4 OS例検証は今回artifactの[小さい証拠抜粋](../../../benchmarks/results/sqlite-public-2026-10-08/logs/related-docs-pr98-evidence.md)で参照できる。PR #98 raw CI logsは4ファイルともhashを記録したが、巨大な完全ログは今回artifactへ複製していない。

## 後続担当の作業

1. PR #99 latest headの4 OS/checksとwebsiteをraw jobsから読み戻し、対象commit、SQLite testcase数、failed/ignored、Onboarding PR #98とは別の実行であることを確認する。e3e0ea3 initial checksは進行中で、4 OS readbackは未確認のまま扱う。
2. generated-versus-manual cost sampleは記録済み。sample scope、per-sample allocation outlier、cache条件、raw resultは[検証結果](../sqlite-public-results.md)、[measurement note](../../../benchmarks/results/sqlite-public-2026-10-08/cost-measurement-note.md)、[summary JSON](../../../benchmarks/results/sqlite-public-2026-10-08/cost-summary.json)にある。共有hostの小標本で性能差を主張しない。
3. PR #99 latest CIの読戻し結果だけを[検証結果](../sqlite-public-results.md)、[progress](../progress.md)、[provenance](../../../benchmarks/results/sqlite-public-2026-10-08/provenance.json)へ追記する。Linux source buildのfuzz/native CLI/extracted local archiveはすでに成功ログを保存したが、これらをPR CIの代用にしない。
4. PR #99のdraft/merge/release判断、PR #98のmerge、Nagi版更新は別に扱う。SQLite APIが未リリースというpublic statusは維持する。

## 証拠の入口

[SQLite public result](../sqlite-public-results.md)がstage別の可視結果と非保証をまとめる。[artifact README](../../../benchmarks/results/sqlite-public-2026-10-08/README.md)はrepo内に保存した小さいevidence setとmachine-readable provenanceを案内する。大量のbuild tree、binary、Cargo cacheは持ち込まない。
