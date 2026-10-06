# SQLite private取得予算の検証結果

2026-10-06。基点は#84を反映したmain `e7aff1d`。[設計](sqlite-acquire-budget-design.md)と[ADR 010](adr/010-sqlite-transaction-boundary.md)を先に保存し、native予算をprivate adapterへ接続した。公開Pool／Options／Txの実装ではない。

## 先行失敗と共通原因

tests-only `d30b4d0`のcompile REDはE0432／E0599の20件でexit101。API未実装で、実行時の予算問題の再現とは区別する。独立レビューで、単に最終timeoutを待つoracleでは「nativeで相対予算をリセット」する誤実装を見逃すと分かった。`30932e0`で、元期限を使い切った後の最初の再pollがReady(Err)であることを固定した。

stock logical待ちだけへ予算を接続した中間source `ae8ad4c`では、`acquire_absolute_budget_survives_logical_wait_to_native_fence`が実行時に失敗した。detach gateでpermit返却と旧native生存を確認し、200msの元期限到達後もnative fenceがPendingだった。3秒のwatchdog、get取消、gate解放、actual join、legacy新取得・rollback、closeを終えてから、`original absolute budget must already be expired at native fence`のassertでexit101。

共通原因は、stockのlogical permit取得をnative予約完了と同一視したこと。deadpoolのwait timeoutだけではnative解放待ちが無期限になる。begin全体をtimeoutにすると、予約済みstartup／BEGINも取り消す別契約になるため採らなかった。これは公開Pool実装前に再現したP1条件で、既存公開Dbの取得障害とは扱わない。

## 変更と継続oracle

一取得のImmediate／絶対deadlineを、stock timeout_getだけのtask-local scopeでManager.createへ渡す。failed／closing、有限予算失効、native容量と登録を同lockで確定する。待機は同deadlineのtimeout_at。登録後にはtimerを持ち越さない。AcquireTimeoutはPoolをfailedにせず、原worker cause・closingの優先、Startup／observerのclose／join責任を保つ。

| 契約群 | 観測 |
|---|---|
| 予算算術 | Immediateとexpired有限を区別。残時間は減少・0へ飽和、表現不能deadlineはNone |
| 0msの空き | startupがPendingでも予約でき、再貸出も成功 |
| logical不足 | AcquireTimeout、worker／Pool状態を壊さず再取得できる |
| native不足 | stock permit返却後も0msは失敗。有限予算は期限前Pending→同期限でtimeout |
| 残予算 | logical待ちで使い切った予算はnative入口の最初の再pollで拒否。workerを増やさない |
| 有限失効と空き | native容量が空いていても新登録しない。次のImmediate取得は成功 |
| 同task・取消 | longをnative fenceまで進め、別予算をpollしても混同しない。取消・actual join後のlegacy新登録は成功 |
| 登録後 | startupとBEGINが期限を超えても、解放後はSQL／rollbackまで成功 |
| cause | Closed／recycle故障をtimeoutへ上書きせず、故障後replacementを行わない |

Sol 2人で設計・実装と独立レビューを分担した。レビューでP2のoracle不足とsetup失敗時cleanupも補強した。予期しない成功Txはrollback、setupのpanic／watchdogはgate解放・close後に失敗とする。元50件のassertを削除・緩和していない。

## ローカル検証

| 検査 | 結果 |
|---|---|
| private native22＋adapter27＋取得9＋比較2 | 60成功、failed／ignored 0 |
| runtime全体 | 189 unit＋5 doctest成功 |
| workspace | 92 suite／891成功、failed／ignored 0。既存High→Low→Rust conformanceとnative統合も実行 |
| fmt／all-target clippy | 成功、警告をerrorとして検査 |
| 既存fuzz smoke | 1000 mutation、95 checked Low emit、16 bounded native、panic 0。SQLite予算のcoverage-guided fuzzではない |
| CI変更判定 | Python52件成功 |
| website | 90ページ、local links／anchors／assets成功 |

[検証artifact](../../benchmarks/results/sqlite-acquire-budget-2026-10-06/README.md)にrawログ・command／exit／source provenance・sampleを保存した。4 OS CIは公開PRの最終headで別に確認する。Linuxのローカル成功から他OSの成功を推測しない。

private warm cap1比較では予算あり／従来fixtureのbegin Futureが2040／2024 byteで差16 byte。Nagi生成Futureやheap allocationの値ではない。p50/p95は82.354/156.686µsと92.219/149.365µs。実行条件と変動を残し、速度向上や無負担を主張しない。

## 保証の範囲と残る課題

privateの上記有限ケースについて、予約予算・原因区別・既存lifecycleの回帰を継続検査できる。TokioのReady優先・executor遅延・non-yielding処理も含む厳密な壁時計上限は保証しない。0msと有限expiredを同一扱いせず、scope無しのprivate比較を公開の無期限defaultにしない。abort／OOM／process killや、受理済みSQLの副作用rollbackも保証しない。

| 残る対象 | 分類・次の行動 |
|---|---|
| 巨大capacity | P2公開前ブロッカー。[stock allocationの判断](sqlite-capacity-decision.md)を終える。恣意的cap・新依存・forkを勝手に追加しない |
| Options／Pool／Failure／Parameters | Phase 4公開配線の未実装。private成功を公開保証へ広げない |
| Tx capture／registry／sealed SQL | 予定46入力をsemantic pass/fail・元位置・High／保存Low／手書きLow・Rust build/runへ昇格する |
| observer内部panic等 | P2検証範囲。close timeout等で観測される条件を公開化前に点検する |
| Phase 4 acceptance／Phase 5／配布 | G-TX／G-POOL acceptance後に次Phaseを判断。版更新・releaseは未実施 |

今回のscopeはcheckの公開保証、compiler、cfg(test)を除くruntime、依存、CI設定を変更していない。未解決の公開設計条件を、隠れた成功やTODOだけで完了扱いしない。
