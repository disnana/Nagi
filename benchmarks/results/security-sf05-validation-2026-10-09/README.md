# SF05 原証跡索引

base main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`の保存WIPを検証し、この索引を含む実装commitでsourceを固定する。[結果日英](../../../docs/internal/security-foundation-sf05-results.md)と[引継ぎ](../../../docs/internal/handoffs/2026-10-09-security-foundation-sf05.md)が保証と未確認範囲を記録する。独立review・このSF05 headの四OS CIは未完了。

| 根拠 | 原ファイル / directory |
|---|---|
| 旧契約の最小RED・全三構文 | initial-red-all-paths.json、initial-red-snapshot、initial-red-provenance、legacy-inventory.json |
| 保存fullと最終source差分 | final-full-offline/provenance.json、workspace.log、scoped-delta.json、final-snapshot.json |
| 最終constructor拒否/default/no-engine | final-scoped/provenance.jsonと各原ログ |
| 最終native三構文9群 | final-native-network/provenance.json、native.log、final-native-network-artifacts（216files） |
| 実SQLite admission/容量/close | final-admission/provenance.json、runtime.log（79件）、public.log（1件） |
| SQLengine無効/registry/SQL opt-in | engine-free、sql-optin-migration-check、sql-lib-checked-plan、isolated-sql |
| fmt/clippy/fuzz | quality/provenance.jsonと各原ログ |
| Node actual196/0skip | editor-final/provenance.jsonとraw.log |
| 移行アプリ10projects/19実行 | application-examples、application-raw |
| 日英SQLite tutorial High/独立Low | sqlite-tutorial、tutorial-raw |
| HTTP8native/84業務check | http-business-combined.json、http-business-complete、http-business-final-closure、http-crud-inventory、http-tasks-result |
| 小さいQuery generated/manual費用 | query-cost-outside、query-cost-artifacts、cost-summary.json |
| 最終日英Docs/website | docs-current-links.json、website-immutable.log、final-snapshot.json |
| commandごとのexit/原log SHA一覧 | run-ledger.json |
| 原artifactのSHA集合 | artifact-manifest.json（manifest自身を除く） |

`source_head`がbase3b8を指すrunは未commitの作業sourceで行ったため、実際の入力を各`source_sha256`へ記録した。final-snapshot.jsonが最終入力集合と各runを照合する。別worktreeで使っていたwarm compiler/native cacheをSF05 source fingerprintで再buildして再利用した。clean buildの証拠ではない。

partial HTTP commandはCRUD/inventoryの4run/36check成功後、後続tasksのfixture import不足でcommand自体が失敗した。別commandで残りtasks/Result APIの4run/48checkを成功させ、compiler binaryと全アプリinput SHAの一致を確認したcombined台帳へ結合した。前半commandのexitを0に変更しない。whole-workspaceのネットワーク取得失敗、初期API移行、Node PATH/sandbox、HTTP fixture、費用fixtureのworkspace配置失敗も元の記録として保持する。parse/import/API未定義/別制限/panicを目的negative GREENへ読み替えない。

成功件数にexisting ignored measurement、0filtered、skipを含めない。所有する小さい通常fixtureのみで、外部攻撃/脆弱性PoC/巨大allocation/負荷や枯渇の検査は行っていない。小さい費用観測は共有Linux host一回のcallerallocation/Futureサイズとclose契約に限定し、時間の有意差や他OS/本番throughputを推定しない。
