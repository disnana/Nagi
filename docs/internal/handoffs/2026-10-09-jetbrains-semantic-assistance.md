# JetBrains意味解析/Run候補の継続引継ぎ

2026-10-09 JST。現在の機能・検証境界は[結果](../jetbrains-semantic-assistance-results.md)、compiler/IDE契約は[semantic assistance contract](../jetbrains-semantic-analysis-contract.md)、手動試用は[GUIチェックリスト](../jetbrains-semantic-assistance-gui-checklist.md)を正本とする。

## JetBrains候補の現在地

- main baseは `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`。#100/#101を含むPR統合状態はGitHub/mainの最新読戻しを正とする。
- [PR #104](https://github.com/disnana/Nagi/pull/104)、branch `feat/jetbrains-semantic-assistance`、worktree `/workspace/Nagi-jetbrains-assistance`。main向けDraft。production/test sourceの検証commitは `3a044fb4e93a56fb3de93afbc7b8f05ad6306a1a`。続くMarketplace配布案内commit `f4bc1620a18422d57394e5a4b3af9946e0388e26` と今回の結果記録はDocs/evidenceのみで、source/test bytesを変えない。
- integrated source local result: Linux generation-retention native48、IC EAP/JDK25のJava70・0 skipと`buildPlugin`、matching compiler SHA-256 `3f278636d81197acf35d320019975e438602e2ea2071d8e7082e965da0800744`、workspace exit0のraw101 blocks/1039 pass/0 fail/1既存ignored、all-target compiler clippy/fmt。raw test countはunique tests数ではない。原ログ・provenanceは[run-retention evidence](../../../benchmarks/results/jetbrains-assistance-2026-10-09/run-retention/README.md)を参照。
- 独立Sol High reviewはgeneration回収の5件を修正後にproductionを承認した。Windows回帰はnightly-only identity APIをstable namespace references/native output bytes oracleへ置換している。Linux結果や以前の`feafba8` CIを最新HEADの4 OS・4 IDE/Verifier成功へ数えない。統合HEADの最終CIは未確認で、rootがPR/API/GitHub側を担当する。
- Plugin 0.1.3は未公開候補。compiler 0.1.12は中止、public compiler 0.1.11は通常Check/Run用でassist protocolを持たない。候補assistは同じPR sourceから作ったdevelopment compilerを必要とする。0.2.0完成前の正式compiler releaseは行わない。
- GitHub Release `jetbrains-v0.1.2`の公開common ZIPとSHA-256はAPIおよびasset endpointで確認済み。Marketplaceのpublic updates/feedは0.1.1のみを表示するが、ownerの0.1.2 approval通知とowner review stateはpublic endpointだけでは照合不能。approval通知を撤回したり、0.1.3承認を推測したりしない。公開案内はnumeric listing ID `34891`を維持し、MarketplaceのVersionsから取得可能版とIDE対応を確認するversion-neutral表現。
- GUIはユーザー提供画面でHigh Run出力/exit0、上部Nagi Run構成、右クリック標準Run項目の表示まで確認。実クリックと実行の因果、IDE/compilerのexact source identityは未確認。補完/navigation/live diagnostics/Low/Stop/trust/project-switchのGUI確認は保留。
- RetentionはIDE-managed Runに限る。foreign-outへexportしたmanaged generationをimmutable CLI成果物同様に保護し、unknown metadata/link/reparse/failed stagingとshared Cargo targetは回収しない。世代数・総容量の厳密なcap、任意Rust include/build-script依存の網羅、電源断耐久、普遍fsync/OOM保証はない。

## 引継ぎ手順

1. rootが最新PR headをpush/readbackし、4 OS/4 IDE、Plugin Verifier、Ready gateとartifact source/hashを最新head上で確認する。PR本文・Draft状態もrootの判断に任せ、旧headの緑を新HEADへ流用しない。
2. GUI追加試用が続く場合はcandidate pluginとmatching development compilerの同一source identityを保持する。現時点の画面で補完・Low・Stop・trustの成功を主張しない。正式release/tag/Marketplace操作は別承認を待つ。
3. Security Foundationの次作業は、既に存在している `/workspace/Nagi-security-sf05` / `feat/security-foundation-sf05` のworktreeを継続使用する。Sol High担当がbase `3b8da226`上で進めており、保存済み全workspace raw1011 passとruntime79の直近結果がある。新しいSF05 worktreeを作り直したり、既存worktreeをreset/cleanしたりしない。
4. SF05のPR/検証を保ち、後続SF02→SF03は採用済み依存順に進める。既存SF01は再実装せず、Security Foundation API変更をJetBrains PRへ混ぜない。
5. Nagi 0.1.12 compiler候補は中止済み。branch/tag/releaseを復活させず、Nagi 0.2.0完了後の配布判断を別に行う。merge/tag/formal release/Marketplace uploadの操作は行わない。

この引継ぎと検証記録は日本語Docs/evidenceのローカルcommit対象。PRへのpush・PR本文変更・Draft解除はrootが行う。
