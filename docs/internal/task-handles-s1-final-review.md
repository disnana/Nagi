# PR #88: Task handles S1最終レビュー

2026-10-06。ユーザーの指示に沿い、[PR #88](https://github.com/disnana/Nagi/pull/88)をS1完成PRとしてレビューした。S2や大規模機能を追加せず、既存契約の文書整合と回帰不足を修正する。その後の最新指示により、S1完了後も別PRで必要なTask残件と次リリース公開へ継続する。

## 基点と範囲

main `9ba4a104be6f65dba61eda0c7b1ef0f98c892cf7`、PR head `a9a0d42e94d6c7518bd7d955c9400299e554888c`、tree `f077a9b1df2d10484cfd8c06e42e82996d2289cc` をGitHubとローカルで読み戻した。この基点の[checks](https://github.com/disnana/Nagi/actions/runs/37492925004)・[website](https://github.com/disnana/Nagi/actions/runs/37492924210)は成功、未解決threadは0件。基点CIを補強後headの成功として数えない。

mainとの差分についてAST/parser/resolver/stdlib、capability/checker、checked facts/封印、High→保存Low→最終check→Rust生成、公開runtime、native harness、CI、日英Docsを照合した。snapshotは歴史的証拠であり現行sourceへコピーしない。既存Sol独立レビューとhashは[接続結果](task-handles-s1-results.md)に保存済み。今回のroot再レビューと、新たなSol独立最終レビューを区別する。

## 契約の照合

| 契約 | 実装・oracle | 判断 |
|---|---|---|
| 公開API / identity | SpawnBind、正式std.task metadata、独立resource inventory、保存Low/raw check、user Task/spawn互換 | Task/Failure/Kind、discard/kind/messageの型・Passing・borrow_ownerを維持。裸名をbuiltin化しない |
| ownership / 義務 | ScopeId/binding義務、move転送、分岐join/loop固定点、元行付きcorpus | 全T一回消費、正常出口await/discard、再代入/shadow/escape拒否。movedだけで義務を代用しない |
| Low / sealed生成 | 保存Low再parse/check、scope/bridge/action破損拒否、三構文native | checker決定を封印planから実現。暗黙cloneやemitter所有権再推論なし |
| 業務Err / 故障 | 独立barrier、public API、runtime oracle | 二重Result、業務Err兄弟継続、sticky fault、legacy primary/body元Errorを維持 |
| lifecycle / cancel / Drop | 実Tokio join、cleanup gate、未poll/Pending受取Drop、取消drain再開、parent Drop | 正常/Error出口の全actual joinと同期Dropのabort要求を分離 |
| discard / close | unit discard、payload Drop場所、未join/未受取保持 | 放棄はdetach/子停止/fault抑制/資源close完了ではない。新しいNagi Task cancel/close APIなし |
| 旧Supervisor / HTTP | 旧scope/Task混在scope、実actor/Supervisor/HTTP、追加三構文oracle | 業務reply後の継続、正常Supervisor停止後のHTTP継続、terminal故障によるHTTP取消/実join |
| Docs / 公開版 | DESIGN/async/syntax/roadmap日英、Task guide、AI資料、ADR012/invariants | S1作業branch・未リリースとS2未実装、旧spawnの契約を区別 |

Taskの静的scope所属と正常出口義務はNagi checkerの保証であり、手書きRust TaskScopeのconsumeだけでは代用しない。rustcのcrate/trait/Send/Sync検査をNagiへ重複実装しない。non-yielding強制停止、外部副作用rollback、任意Rust Drop/panicの普遍回復は保証しない。

## 発見・修正

**P2・negative不足（補強済み）**。discard→await、match一部継続枝の未消費、while複数周回consume、直接Task copy/share/view/print、class field/enum payload宣言の拒否を9 High/Low対追加した。既存期待を変更せず、checker段階＋fragment＋primary元行を要求する。match両枝消費→while再生成/受取のpositive対も追加し、両flagを三構文で実build/runする。

**P2・service専用oracle不足（補強済み）**。旧sampleの成功だけではterminal故障→HTTP停止の実証にならなかった。`compiler/tests/support/task_service.rs` を既存harnessへ接続し、旧spawn-onlyとTask混在scopeの各々で業務reply、terminal failure、正常shutdownを実行する。bind済みlistenerを実 `http_server::serve_listener` へ渡し、terminal時はbodyがErrを返さずmonitorの旧spawn Errがscope出口故障になる。HTTP Future Dropとlistener停止を出口前に確認する。正常停止では503を実socketで受けてHTTP継続を確認し、別signalでHTTPを終了する。このoracleはterminal前に要求を完了しており、pending handlerのDrop完了をこのtestで観測したとは説明しない。

**P3・Docs不一致（修正済み）**。英語DESIGNの業務Err表、日英roadmap、AI資料前半に「未実装/将来のみ」が残っていた。S1実装済み・未リリースへ揃え、std.task登録と旧statement spawnの返却型/Errを明記した。syntaxのscope return/view拒否と旧spawn型制限を日英で分けた。runner reportの「static Task guaranteeなし」という古い説明もchecker所有権検査へ訂正し、native未実行とは区別した。

production compiler/runtime・Cargo依存・版・旧Supervisor/HTTPを変更する新不具合は今回のrootレビューで確認していない。接続source `34ac4d5` の記録済みproduction/manifest 42 hashesと本体差分を照合し、不変を確認した。任意入力の正しさを証明したという結論ではない。

## 補強後の検証と公開条件

契約は **110/110（55 High/Low対、40受理・70checker拒否）**。追加18 negativeは指定したchecker診断/元行で拒否、追加2 positiveも受理した。Task nativeは6群となり、全20 positive対の三構文、barrier lifecycle、seed16経路×両flag、service三構文を含む。runtime17・公開API3・runtime doc9の期待は維持する。

全workspaceは95 result block・934成功・failed0・費用用ignored1、fmt・all-targets clippy・runner oracle3・登録が成功した。Markdownは213文書/1937 local links、siteは92ページを検査して欠落なし。release unitは98/98、検証用Linux release archiveの既存検査全体と新Task三構文/12 early negativesも成功した。archive内runtimeへの依存を確認してcheckout fallbackを防ぐ。新たなSol独立レビューはproductionの追加不具合なし、Docs三指摘の修正確認、配布Task三構文/12拒否の別実行を完了した。Linux archiveは0.1.10表記・a9 sourceの検証用であり、新release公開ではない。

原ログ/hashは[最終review artifact](../../benchmarks/results/task-handles-s1-2026-10-06/final-review/README.md)へ。意味論/測定fixtureが不変のため費用測定は重複せず、[既存条件と限界](task-handles-s1-results.md#同条件の生成rustと手書きrust)を維持する。

最新公開headで必須CI・4 OS・website・独立レビュー・未解決thread・mergeabilityを読戻してからS1完了を報告する。次のTask工程と最新のリリース承認範囲は[完成後引継ぎ](handoffs/2026-10-06-task-handles-s1-complete.md)へ分離する。
