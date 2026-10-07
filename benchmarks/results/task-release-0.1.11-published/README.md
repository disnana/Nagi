# Nagi 0.1.11 最終CI・独立レビュー・公開後確認

正式公開した[Nagi 0.1.11](https://github.com/disnana/Nagi/releases/tag/nagi-v0.1.11)の最終確認。公開sourceはmain/tag `003a594de086383100016b7c75466da37705646c`。PR #91 head `c2b227533b89268028ecb2d67e39492b7aa4e4a5`、local `64a7f3d5c5306abca579e5d95dd9990087747699`と共通tree `13b41a034bf463a9426c53327b42c4184a53c7ed`。この追記でsource・版・配布assetsを変えない。旧候補のCIと修正PR #92のCIを最新版の実行へ数えない。

- `independent-final-review/`: Sol 6.1 Maxによる最終凍結候補の独立review。条件付きTask消費の修正・互換移行・版差分・API・Docs・source/artifact hashを確認。指摘2件を修正し未解決0。継承したnative/全workspace/4 OSとreviewer自身の今回実行は報告で区別する。
- `release-pr-ci/`: 最終PRの4 OSとLinux全workspace原ログ・oracle validator。各platformの148契約、Task native8、runtime17、public3、doc9、両公開例の三構文、展開archive版0.1.11とTask gateを照合する。費用用ignored1は実行へ数えない。

- `release-main-ci/`: 公開sourceのchecks `37558874886`・website `37558874635`は成功。各4 OSの148契約・native8・runtime17・public3・doc9・両例三構文・展開archive版/Task gateを原ログとvalidatorで確認。Linux workspaceの95 result blocksは936成功・failed0・費用ignored1、fuzz1000/panic0/bounded native16。公開job `112597346200`も成功。IDE/VSIX/Readyのmain skipを実行へ数えない。
- `published-readback/`: `published_at=2026-10-07T02:11:53Z`、draft/prerelease=false、latest、tag/source一致、公開commitのCHANGELOG/前版0.1.10とのnotes比較。実8assetsのdownload・byte/SHA-256/GitHub digest、4sidecar・release.json・standalone runtime0.1.11を検証。公開binary/cacheはrepositoryへ含めない。
- `published-readback/linux-public-archive-verify.log`: 公開Linux archiveをcheckout外で既存verifierに通しexit0。同梱runtimeと実native pathのTask三構文/12拒否、受取/discard/業務Err分離、SQL/actor/local Rust/JSON/alias/元位置を確認。26.849秒、依存cache再利用でありclean buildではない。macOS/Windowsの公開assetsはhash/identityを確認し、実行成功は公開main各OSのarchive gateとして区別する。

`provenance.json`がsource・公開readbackと、このfolderの保存artifact SHA-256を記録する。生ログは原文を保ち、結果集計では子image再実行の重複・ignored・filter・skipを区別する。既知のTask release blockerは0。保証の限界と次PR候補・推奨順は[最終引継ぎ](../../../docs/internal/handoffs/2026-10-07-task-release-0.1.11.md)へ保存した。
