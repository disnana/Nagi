# PR #102: 2.19実SDK検証の保存資料

[最後の差分の独立レビュー](final-close-review.md)は、追加した負例・結果文書・保存資料を読み戻し、blocking指摘なし。最終commit後のChecksは別に確認する。

[検証資料アーカイブ](evidence.zip)はプラグイン配布ZIPではない。SHA-256は `b354c7cfec027fb3ad478d0b39103986d9fa8e99cd25f4ac1a58c807f1c13030`、237,039 bytes。展開後の `evidence-manifest.json` に各fileの原byte hashを置く。保存command/log/metadataは当時のsnapshotで、古いpendingレビューを現在のCI成功に読み替えない。

- source `52177e848410a3800d3a9ea34327f7b2cd3ecc65`、base `c3e5fc7a795d66c8f6408ac12b480230dd3a8973`。compiler/runtime/Cargoにmainとの差分なし。`source-head-52177e8.json` に変更file hashとeditor subtreeを保存。
- `ci-37802356248.json` は[成功run](https://github.com/disnana/Nagi/actions/runs/37802356248)のraw REST run/jobs/artifacts。`ci-logs/` はcandidate、四SDK、元candidate bytesの配布gateの原ログ。四SDKすべて40成功・失敗0・skip0、Verifier Compatible。Linux全回帰と四platform nativeもsuccess、publishのみPRとしてskip。
- `review.md` は独立Sol Highの読み取りreview。公式2.19のsource blob IDs、Java21/SDK25、ZIP gate、headless trust修正の根拠と当時の未確認範囲を含む。
- `local-ic-eap-junit-2.19/` は39成功1失敗のRED、`isolated-trust-observation.xml` は実際のtrustがtrueだったことを示すRED。`isolated-trust-green.*` と `local-ic-eap-green-2.19/` はtest-onlyのheadless shortcut無効化後の1/1・40/40 GREEN。元assertionを保持した。
- `final-trust-path-preconditions.*` はeligible local sourceと期待した拒否dialogもassertした最終負例の1/1 GREEN。`final-trust-preconditions-source-manifest.json` はそのtest/CI guardのsource hash。後続の文書commit SHAを自己参照させず、最新CIは[PR #102 Checks](https://github.com/disnana/Nagi/pull/102/checks)で読む。
- `trusted-projects-primary.kt` は関連公式master source、`trusted-projects-javap.txt` は実IC EAP SDK bytecode。両者を同一revisionとは主張しない。`root-test-framework-deprecation.log` はtest-onlyのDisposer.isDisposed warningを特定したcompile-only実行。

ローカル40件中の実compiler smokeは別assistance worktree由来の既存debug binaryを使った。対象HEADのcompilerと数えず、exact sourceから再buildしたCI側のHigh/Low/project smokeを根拠にする。CI outer artifact digestをinner plugin ZIP hashへ流用しない。inner hashの直接取得は403で未確認だが、CIの各候補checksumと最終 `nagi-jetbrains-0.1.1.zip: OK` は成功した。GUI手動操作・Marketplace rendering/upload・releaseは実行していない。

公開asset/Marketplaceの根拠と履歴は[結果記録](../../jetbrains-release-pipeline-results.md)、次順序と制約は[引継ぎ](../../handoffs/2026-10-09-jetbrains-unification.md)を参照。
