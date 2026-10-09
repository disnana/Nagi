# JetBrains意味解析/Run候補の継続引継ぎ

2026-10-09 JST。[結果](../jetbrains-semantic-assistance-results.md)、[契約](../jetbrains-semantic-analysis-contract.md)、[GUI手順](../jetbrains-semantic-assistance-gui-checklist.md)を正本とする。

- main確認値 `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`。#100/#101/#102/#103はmerge済み。rootによる今回のmergeはなし。
- [#104](https://github.com/disnana/Nagi/pull/104)、branch `feat/jetbrains-semantic-assistance`、worktree `/workspace/Nagi-jetbrains-assistance`。main向けDraft・未merge。HEADはPR APIから読戻す。
- compiler正本の補完/navigation/編集中診断、常駐worker、標準右クリックRun/上部Run/Stop、固定PR HEADのCI候補artifactを実装した。0.1.3は未公開の候補。公開0.1.2/ID/Marketplace更新経路を維持する。
- ローカルcompiler17、IDE69/0skip/buildPlugin、workspace1012 passed/0 failed/1既存ignored、fmt/clippy/fuzzとDocsを確認。source/log hashesは保存artifactへ収録。
- 独立Sol HighのP2 3件を修正/再確認済み。Windows verbatim source keyの追加差分と最新HEADの4 OS/4 SDK/Verifierは最終PR/Checksで確認。ユーザーWindows GUI試用は未実施。
- GUI用はCIの `release-jetbrains` + matching `release-windows-x86_64`。同run/head/内側checksum/compiler `release.json.commit`を確認。古い公開compilerへ新補完があると案内しない。
- 本体0.1.11/VS Code0.1.13の版は維持した。タグ/正式release/Marketplace uploadなし。Getting StartedのWeb UI本文管理はユーザー所有で、同期fileは作らない。

## 次の順序

1. #104最新headの全必須CI/API verifier成功とartifact source/digestを読戻し、PR本文へ結論を記録。
2. userに両IDE共通候補ZIP、matching Windows compiler、右クリックRunを含むGUI試用手順を渡す。merge/releaseはGUI承認まで行わない。
3. JetBrains開発検証後、別worktree `/workspace/Nagi-security-sf05` / branch `feat/security-foundation-sf05` を最新mainから作りSF05を再開する。SF02を先行しない。
4. SF00 RFC/D1–D3/移行計画と現行SF01実装を読み、SF05のpublic Query/Parametersとprotected predicate責務をRED前に固定する。旧APIを安全性迂回用に残さない。
5. SF05のHigh/保存Low/手書きLow/native/元位置・移行同等業務動作、回帰/4 OS/日英Docs/独立reviewを別Draft PRで完成する。0.2.0版/tag/releaseは別承認が必要。

既存 `Nagi-security-foundation` (`a97c7fb`) / `Nagi-security-sf01` (`07e17fc`) は過去検証の保全treeとして残す。
`/workspace/Nagi` local main/refは古いので、そのrefを最新としてcheckoutしない。fetch mainはFETCH_HEADだけを更新するremote設定に注意する。
過去SF01結果のDraft/未merge記述は当時snapshotで、現在のGitHub merge状態を優先する。
未マージPR/#104へ無関係機能を入れたり保全WIPをresetしない。
