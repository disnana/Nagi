# SQLite公開APIの4 OS CI読み戻し

対象source: `e86a9419fbcc547e937a13f0c0bf3927f98053b8`。compiler/runtime/Cargo.lock/SQLite配布gate/費用harnessは既存検証source `e3e0ea3`と同一。後続のDocs/artifact-only commitのCIをこのheadの実行と同一視しない。最終PRのheadと必須Checksは[PR #99](https://github.com/disnana/Nagi/pull/99)本文へ記録する。

- [checks 37706126168](https://github.com/disnana/Nagi/actions/runs/37706126168): attempt 2、success。publish-releaseは意図したskipであり、実行成功に数えない。
- [website 37706125992](https://github.com/disnana/Nagi/actions/runs/37706125992): 同head、success。
- [push checks 37706121800](https://github.com/disnana/Nagi/actions/runs/37706121800): 同head、success。
- [API読み戻し](logs/ci-e86-final-readback.json)、[原ログhashと圧縮file](ci-raw-log-hashes.json)。ログはGitHubのdecoded UTF-8でCRLFをLFへ正規化した。

| 実際に成功した実行 | job ID | 原ログ |
|---|---|---|
| Linux全回帰 | 113081255940 | [gzip](logs/ci-e86-113081255940.log.gz) |
| Ubuntu package | 113081297043 | [gzip](logs/ci-e86-113081297043.log.gz) |
| Windows package | 113081297039 | [gzip](logs/ci-e86-113081297039.log.gz) |
| macOS arm64 package | 113081297118 | [gzip](logs/ci-e86-113081297118.log.gz) |
| macOS Intel package、一度の再実行 | 113090439243 | [gzip](logs/ci-e86-113090439243.log.gz) |

各package logでSQLite契約46/46、sqlite_public 8/8、SQLite runtime 73/73、Task契約148/148、初アプリ10native、SQLite教程High/独立保存Low、展開配布物のSQLite Pool/Tx native gateを確認した。Linux全回帰にはworkspace cargo test、examples 10 projects/19 runs、fuzz 1000 text mutations/16 bounded native/77 checked Low mutationsが含まれる。IDE/VSIX/merge gateもsuccess。attempt 2で新job IDを持つ先行成功jobはコピーされた状態であり、実行し直したものとして数えない。

## 初回Intel失敗と再確認

初回job `113081297091`はcompiler/CLIを通過後、editor `stdlib-navigation.test.js`のHigh/Low symbol取得2件が既存5000ms期限でSIGTERMとなり199/201、skip 0だった。[失敗原ログ](logs/ci-e86-113081297091-failure.log.gz)を保持する。診断内容の不一致ではないが、infraだけの問題と断定しない。

同一headで失敗jobのみ一度再実行し、期限・並列数・assertを変更せずeditor 201/201と後続native/package検証が成功した。これは原因解決や将来の時間上限の保証ではない。再試行の前後でproductionは変更していない。

通常サイズの同じfixtureをLinuxで各3回逐次実行すると、mainと同じcompiler sourceは約0.43–0.46秒、今回sourceは約0.56–0.60秒、JSONは約218KB→233KBだった。[測定JSON](logs/editor-symbols-comparison.json)、[調査記録](logs/editor-timeout-investigation.md)、[baseline build](logs/editor-main-baseline-build.log)、[期限を変えないlocal test](logs/editor-timeout-local-focused-corrected.log) / [TAP](logs/editor-timeout-local-focused-tap.log)を参照。これは小標本の費用観測で、macOS failureの原因を特定した証拠ではない。Symbols経路はfinalization replayへ入らない。symbol/catalog経路のprofilingとCI時間変動調査を後続候補とする。負荷試験・巨大allocation・資源枯渇実験は行っていない。

## mainへの移行

CI待機中にPR #98が外部でmainへmergeされた。main commit `2d87d8354808b221d537a8b1555ca39d2b95ef89`と旧base `bc6a76bf2422aadaca799b739ab75e5f1e6bc450`のtreeは同じ`4d72a7d39ccd164400ce6a0e1353437b637f0ce7`。PR #99のbaseをmainへ変更し、mergeableを読み戻した。ソースを変更するrebaseは不要だった。このagentはmerge/release/version bumpをしていない。
