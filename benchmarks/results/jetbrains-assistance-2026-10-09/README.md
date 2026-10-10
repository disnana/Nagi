# JetBrains assistance候補の検証記録

[契約と制約](../../../docs/internal/jetbrains-semantic-analysis-contract.md)・[GUI試用手順](../../../docs/internal/jetbrains-semantic-assistance-gui-checklist.md)・[PR #104](https://github.com/disnana/Nagi/pull/104)。正式release artifactではない。

`review-red.log`は未知closed dependencyの旧factsと同path compiler更新時の旧process reuseの契約RED。
`review-green.log`は修正後66 IDE tests。`review-final-build.log`は追加native/missing-importを含む68 testsとbuildPlugin成功。
`retry-final-build.log`と`retry-refresh-build-corrected.log`はretry sequencing修正後の全68件、続いてcallback共通化後の4件とbuildPlugin成功。
`windows-source-identity-red.log`はRust verbatim Windows pathとIDE source key不一致のRED。`windows-source-identity-green.log`は正規化後の全69件とbuildPlugin成功。Windows GUIでの実証ではない。
`workspace-reviewed.log`は全workspace（99 result blocks/1012 passed/0 failed/1既存Task cost ignored）、重複子processの扱いはログを正とする。
`sqlite-busy-fixture.log`は既存0ms Busy fixtureの返却barrierのみのtargeted成功。SQL public API/runtime意味論は変更しない。
`clippy-reviewed.log`はworkspace all-targets成功。

performance JSONはoptimized compilerのbounded 7-run測定。10/100/500 localsの常駐protocol p50は約166/182/318ms。IDE描画・debounce・cacheを含まない。
`cache-measurement.log`はPlatform fixtureのcached response検証。最新69件のJUnit XML/`ide-summary.json`では500 lookupのmean約0.133ms/追加launch0を観測し、実GUI end-to-endとは区別する。
同時Gradle/build中・Linux共有環境・warm target/cacheの測定で、one-shot測定とは実行時刻と競合条件が違う。数値SLAはGUI試用後に決める。

各raw logとsource hashesは`provenance.json`。CI成果物は候補PR HEADをcheckoutして作る。最新4 OS/4 SDK/Verifierの状態とartifact run/head/digestはPR本文とActionsから読み戻す。ローカル記録や過去CIを最新HEADの実行成功として代用しない。
