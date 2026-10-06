# SQLite private取得予算: logical slotからnative予約まで

2026-10-06。基点はmain `e7aff1d`（#84）。Q002／Q004と[ADR 010](adr/010-sqlite-transaction-boundary.md)の取得・終了契約を、cfg(test)のadapterで検証する。公開Pool／Optionsの配線、任意の数値上限、依存変更はこの縦切りに含めない。

## 保持する契約

- `acquire_ms`は予約待ち。一つの取得でlogical permit待ちとnative容量の解放待ちへ同じ予算を使う。段階ごとに時間を足し直さない。
- 0msはImmediateとして扱う。logical permitとnative容量が直ちに取れれば成功し、条件が成立しなければ待たずに失敗する。0msを「必ず期限切れになる過去のdeadline」に変換しない。
- native容量・closing／failedの確認とlive record登録は同じledger lockで行う。登録後のready、open、BEGIN、busy、SQL、返信には取得タイマーを持ち越さない。
- 取得期限切れは`AcquireTimeout`／`NotApplicable`／`retired=false`。worker障害やcleanup失敗に変換せず、Poolをfailed／closingにしない。既に公開されたworker causeとclosingは従来の優先を保つ。
- 取消が登録より前なら新workerを起動しない。登録後はStartup／observerがcloseとactual joinの責任を保持する。
- privateの従来の無期限helperは回帰比較用に残す。公開Optionsへ無期限sentinelやdefaultを追加するものではない。

## 内部表現とcrateの分担

一取得の予算はImmediate、または一つの絶対deadline。duration入口は最初のpollで一度だけdeadlineを作る。native fenceは同じdeadlineを受け取り、Notifyの待機をその期限へ結び付ける。純粋な残時間計算も固定入力で検査する。

deadpool 0.13.1の`Manager.create`には取得ごとの引数がない。`timeout_get`を一つのTokio task-local scopeで包み、Managerがpollされるたびにそのscopeの値をcopyする。scopeは一取得に限定し、共有mutableな「現在の期限」を置かない。native thread、spawn先、別取得へ渡さない。`try_with`のclosure内でawait／再pollしない。

stock `Timeouts.wait`には残時間を渡し、create／recycle timeoutはNoneにする。stockのslot選択、公平性、recycleを維持する。recycle故障は既存cause公開・Pool停止を維持し、期限を作り直してreplacementする逃げ道を加えない。

有限予算ではfailure／closing確認後、native recordの新登録前に期限を確認する。期限切れなら、容量が空いていても新workerを登録しない。Immediateの場合は空き条件を先に確認する。これは予約未完了の範囲で同じ予算を保つ内部判断で、新しい数値policyではない。

この検査点とstockのReady優先動作は分ける。Tokio Timeoutはinner FutureがReadyならtimerより先に返す。健康なworkerの再貸出、executor遅延、non-yielding処理も含む厳密な壁時計上限は保証しない。これは予約条件の待機予算で、処理時間の上限ではない。

## 不採用

- begin全体のtimeout、create／recycleへ同じ相対時間を毎回設定する案。
- Managerの共有fieldへ取得中の期限を置く案。
- eager prefill、独自pool／公平性algorithm、timer polling、Tokioのテストfeatureや新依存の追加。
- timeoutでclosingを解除する、native recordをjoin前に削除する、cleanupを中断してpermitを返す案。

## 先行oracleと検証順

1. 設計とADR参照を保存する。
2. 0msの空き／logical不足／native不足、有限予算の引継ぎ、同taskの二取得と取消、登録後startup、recycle causeを小さい先行テストにする。未実装APIによるcompile REDと、期限を失った実行時REDを区別する。
3. nativeが残っていることはpublication／detach barrierとledgerで観測する。cleanupを終えてから失敗をassertし、失敗テストでもthreadやfixtureを取り残さない。
4. 予算を接続し、元50件、runtime／workspace、fmt／clippy、既存compiler conformance／fuzz、同条件の比較測定、4 OS CIを確認する。
5. 実行ログと未確認範囲を別の結果文書へ記録する。private試験だけでG-TX／G-POOLやPhase 4 acceptanceを完了扱いしない。

一次根拠: [Q002のAPI](sqlite-pool-proposal.md)、[deadpool pool.rs](https://docs.rs/crate/deadpool/0.13.1/source/src/managed/pool.rs)、[PoolError](https://docs.rs/deadpool/0.13.1/deadpool/managed/enum.PoolError.html)、[Tokio task_local](https://docs.rs/tokio/1.53.1/tokio/macro.task_local.html)、[Timeoutのpoll](https://docs.rs/crate/tokio/1.53.1/source/src/time/timeout.rs)。crate sourceの照合と、実行結果を区別する。
