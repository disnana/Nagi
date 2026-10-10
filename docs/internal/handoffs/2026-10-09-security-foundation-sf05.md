# SF05 実装者から独立reviewへの引継ぎ

2026-10-09 UTC。worktree `/workspace/Nagi-security-sf05`、branch `feat/security-foundation-sf05`、base main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`。この記録を含む実装commitを不変のreview対象とする。SF00 #100 / SF01 #101はmainへ反映済み。親agentがpush・Draft PR・四OS CIを管理し、Engineer終了後にSol High Independent Reviewerを起動する。merge/版/tag/release/upload/公開は未承認・未実行。

## 契約とreview対象

[SF05契約](../security-foundation/sf05-contract.md)、[英語契約](../security-foundation/sf05-contract.en.md)、[ADR 014](../adr/014-literal-query-and-sqlite-admission.md)、[削除・移行inventory](../security-foundation/sf05-migration-inventory.md)を先に読む。採用済みD1–D3を旧API互換性だけで再検討しない。

canonical `stdlib:std.db.sqlite` の直接literal constructor、opaque Copy Query、Query必須のquery/all/exec、checked planからの生成を確認する。旧Db/db_*とruntime public Db/Sqlは実行経路を削除しchecker migration診断だけを残す。同名ユーザー定義を標準APIとして占有しない。通常checkにSQLengineを必須化せず、opt-in collectorは同じSQLite engineへ直接Query/Parametersの既知factsを渡す。未知dataflowを検査済みと報告しない。

実Grant subject/targetを実SQL Parametersにbindするreviewed adapterと同Tx owner/id predicateがnative oracle。Tx.reserve_execは実bounded queue容量だけを予約するtrusted Rust境界。Grant.submitはSF01のprivate一回permit発行で線形化し、同期callback内のenqueueに接続する。発行前の失効は実enqueue0、発行後は実send前の失効も受理済み。Query/任意adapterそのものはtenant制約の証明ではない。取消/後の失効が受理済み副作用を取り消す保証は追加しない。

DDL/seedはレビュー済み固定startup管理処理として、HTTP登録前のbootstrap helperへ分離した。request handlerへ動的SQLfactoryを再exportしない。既存authorizerを維持し、不要なDDL禁止やRust専用bootstrap APIを追加しない。SF02永続Session/世代、SF03 CSRF/CORSはこのPRでは完成させない。

## 実行済み証拠と後差分

[日英結果](../security-foundation-sf05-results.md)と[証跡索引](../../../benchmarks/results/security-sf05-validation-2026-10-09/README.md)を参照する。原ログ、入力SHA-256、command/exit、compiler binary identityを保存した。warm cacheの再利用でありclean buildではない。取得タイムアウト、sandbox/PATH、初期移行・harness失敗は保存し、成功へ読み替えない。

- 保存full workspaceはexit0、raw1011成功/failed0/既存measurement1ignored。compiler/runtime236filesと現sourceを比較しproduction差分0。後差分はconstructor拒否testとSQLite admission testの2filesだけ。
- 後差分はconstructor default/no-engine各12群、runtime SQLite79/publicAPI1で確認済み。最終native9群のHigh・元High削除Low・手書きLowは216原artifact付き。各目的negativeはcheckerと元行で拒否し、parse/import/API未定義等を目的GREENに数えない。
- fmt/clippy/fuzzはexit0、fuzz1000text/77checkedLow/16boundednative・panic0。Node22files/196actualtests・fail0/skip0。manual VS Code host GUIは未実行。
- 移行10projects/19High-Low native、日英SQLite tutorial7/closed、HTTP CRUD/inventory/tasks/Resultの8native runs/84小業務checksを確認。HTTP前半commandは後続fixture失敗を持つpartial runで、成功4runの原結果と後半successful command4runをmatching compiler/source hashで結合した。
- release/x86_64 Linuxの4warmup/32samples限定費用比較はgenerated/manualとも未pollFuture392B/408B、calling-thread allocation2回/280B。rollback/actual close成功。共有host一回の時間、workerallocation/他OS/本番性能は保証しない。新thresholdは設けない。
- website102pagesのlinks/anchors/assetsと最終相対リンクを確認。DESIGN/ADR/日英Docs/examples/migration/CHANGELOG Unreleasedを同期した。歴史的設計・測定の本文はsnapshotとして維持しsupersedes noteを追加した。

最終snapshotの`compiler_runtime_sha256`、`run_identity`、`scoped-delta.json`で保存fullとの差分と各最終runの一致を確認できる。artifact-manifest.jsonは自分自身を除く原artifactのSHA集合。入力manifestはbenchmarks/results以下とpycacheを除き、自己参照を避ける。

## 親agentの次の操作

Engineerを停止した同一immutable commitへ独立Sol High reviewを実施する。指摘があればreviewerを止めEngineerへ返し、修正差分と必要検証を別commitへ記録して再reviewする。四OS CIはこのSF05 commit自身の実結果を読む。CIにはchecker12/native1/admission6のexact count/0ignored、SQLite79、source/log/nativeartifact SHAの登録を追加したが、登録のみを実行成功としない。最新headのreviewと四OS結果を取得するまでReady/merge-readyとは扱わない。Jet PR104の証拠は流用しない。

追加の承認待ち公開仕様案はない。依存順はSF05 → SF02 → SF03 → SF04/SF06 → SF07 → SF08。SF05受理前に後工程へ実装を拡張しない。source・原ログ・warm targetを保持し、dirty破棄/reset/rebaseや歴史的artifact削除を行わない。


## 独立レビュー後のDocs修正

local `c33e8c9`を別Sol Highがreviewし、source/runtime blockerは未発見。permit発行とenqueueの順序、英語DESIGNの移行記述にP2/P3を見つけたため日英Docsを修正した。review-docs-correction.jsonが後差分を記録する。元final-snapshot.jsonを修正後の全入力snapshotと呼ばない。compiler/runtime/tests/依存/APIは変わらず、保存回帰を再実行していない。Docs独立再確認と最新headの4 OS CIを読む。引き続きSF05→SF02→SF03の依存順を保ち、merge/公開操作は行わない。

845d938の独立Docs再確認でSF05-R01/R02を閉鎖し、source/Docsは承認相当。独立追加testのraw file保存はなく、実装者artifactとは分ける。最新headの4 OS CIは未確認。
