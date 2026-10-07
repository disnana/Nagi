# Task/spawn: #94後の引継ぎ

更新: 2026-10-07。最新main `97b7242c2172c1d0c701a7b662294e4dbe268abf`をbaseとする`test/task-spawn-post94`の記録。停止地点はmain向けPRの検証・作成までで、merge/release/version bumpの実行承認はない。

最初にrootの[AGENTS.md](../../../AGENTS.md)、[今回結果](../task-spawn-post94-results.md)、[0.1.11公開時の引継ぎ](2026-10-07-task-release-0.1.11.md)を読む。Taskは#88 S1、#90 S2、#92条件付き消費までmain反映・0.1.11公開済み。古いprivate bridge/50 parse REDのStage 1引継ぎは歴史的記録であり、再実装の指示ではない。公開tagと最新main/PR headを実際に読戻す。

今回の変更は#94との統合回帰3群、Task/moveの公開状態を揃える日英DESIGN/Docs/AI資料、結果・原ログ。production compiler/runtime/API/依存/版/CI定義はbaseとの差分0。三構文のTask結果と129項左結合/右括弧、Taskから後方不正Result署名を参照したときのchecker元位置、import統合Low >2 MBと保存Low再入力の別予算を固定した。新しいRED→GREEN実装とは報告しない。

checkerのscope/binding義務とcanonical std.task metadata、私有sealed生成、High→Low→Rust、保存Low/手書きLow、内側業務Resultとsticky TaskFailure、cancel要求とactual join、discardと同期Drop、旧spawnとSupervisor/HTTPは接続済み。既存契約148/148、runner oracle3、追加後frontend13、native/public/runtime・全回帰・例・fuzz・Docs・4 OSの根拠は今回結果、artifact、PR Checksに分けて残す。過去CI/skip/filter/ignoredを今回の実行成功へ数えない。

同保証の生成/手書きRustを今回再測定し、6条件のallocation/byteが一致、batch Future受取416 B/discard408 B。runtime保持oracleも明示実行した。共有host・cache使用・calling-thread限定であり、multi-thread全allocation/他target/clean build/長時間網羅は未測定。独立Sol High reviewはP3の公開状態表現を修正後、未解決0。reviewerによるテスト独立再実行はない。

## 次の順番

1. このPRの実際のhead、main、4 OS CI、reviewを読戻す。今回の依頼ではmergeせず停止する。mainが進んだらproduction/test hashを照合し、他人の変更を上書きしない。
2. Taskの採用済み実装を重複追加せず、Task/spawn PR完了後のSQLite Pool/Transactionを別工程で扱う。[capacity判断](../sqlite-capacity-decision.md)と専用DB契約/ADRを先に確認する。
3. Taskの任意の改善は、大batch/長body/保持量・multi-thread測定、生成探索と縮小corpus、観測根拠のある内部整理の順で小PRにする。既存意味論とsource/DoS防御を維持する。
4. 個別cancel/close/detach、scope外Task、一般Future保存、公開cause/recoveryは新仕様の判断が先。sticky faultや全T正常出口義務を再質問して既存作業を止めず、採用済み契約を勝手に拡張もしない。

同期parent Dropはabort要求まで、non-yielding強制停止なし、外部副作用rollbackなし、任意Rust Drop/panic payloadの普遍回復なし。これらを未完成の採用済み機能として自動実装しない。#94の極端な平坦ASTに残るstack制約も別の根拠・予算設計なしに緩めない。進捗正本は[progress](../progress.md)。
