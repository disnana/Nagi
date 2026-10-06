# PR #87 private SQLite closeの独立Solレビュー（最終）

2026-10-06、read-only。AGENTSを確認し、最終source `f6bc74a979ed9d83ef15def449d30b93b8e7bb6b`（tree `830c8c27ab50c45257a428d7ecbde3805682346a`）の `runtime/src/sqlite_prototype/adapter.rs:158` request_close、`adapter_tests.rs:165` 先行回帰、WorkerHandle/CheckoutOwner/Startup/observer/native cleanupを照合した。依存はcached deadpool 0.13.1 managed pool/objectとTokio 1.53.1 semaphore。コード編集・Cargo・GitHub操作・Task実装なし。

## 結論と実行の区別

現在の閉鎖経路とfixtureに具体的なblockerは見つからなかった。close→retain(false)→removed Dropはstockの残idleだけを退役し、active checkoutや独立actual join責任を奪わない。期待をCloseTimeout許容へ変えたり、worker joinをsender通知/counter操作で偽装したりしていない。

RED/GREEN/fullの実行担当はrootで、このレビューは実行結果ではない。tests-only `0051f7a`のroot実行原ログ `/tmp/nagi-explicit-move-proof/pr87-sqlite-close-red-clean.log`を読取確認した。回帰は終了前にCloseTimeoutを観測し、Pool Drop後のactual join cleanupを通過してから主assertで失敗した（0 pass/1 fail、2.00秒）。source修正 `a4883487e63b10df0eea3318cf04da6409d2ef33`での全回帰成功163.8秒はrootからの報告。最終fixture修正f6bcの全回帰はレビュー時点でroot実行中であり、このメモで成功とは扱わない。

決定的fixtureはstock APIで同型の終了漏れを実証している。CI run `37454227747`/head `68d215b`のCloseTimeoutが、この特定interleavingによるものだったと一意に同定した証拠とは分ける。元runtime189反復7回の成功も経路不存在の証明にはならない。

## fair semaphoreによるoracle

cap1でHeldCheckoutを保持し、別begin Futureを一回だけpollしてlogical semaphore Pendingにする。HeldCheckout DropはObjectをidle queueへ戻し、Tokioの公平semaphoreはpermitをその既存waiterへ割り当てる。waiterを再pollしないためObjectは取り出されず、resize(0)のtry_acquireは予約済みpermitを取得できない。閉鎖後の再resizeはclosed semaphoreによってno-opになり、旧sourceはidle senderを残し得る。

Tokio batch_semaphoreのadd_permits_lockedがqueued waiterに割り当ててから余りをavailableへ戻す実装と、deadpoolのpopがpermit await後であることに一致する。sleepや真の並行return raceを待たず、stock/dependency改変やnew seamも不要。wakeupはそのFutureを自動pollしない。

主oracleはwaiterの再pollより前にclose結果とafter_close snapshotを保存し、native_closed=1/joined=1/pending_workers=0を要求する。waiterはKind::Closedを要求し、成功Tx/watchdogを許容しない。最後もcreated/native_started/native_closed/joined=1、start_failed/starting/pending_workers=0を保持する。close後のPool Dropだけでjoinした結果を主oracleの成功へ読み替えない。

## retain、lock、ownerと取消

- deadpool pool.rs:305のretainはslots lock中にpredicateとManager::detachを呼び、removed Vecを返す。constant false predicate自体は非blocking。removed WorkerHandleのDropはretainが戻ってslots lockを解放してから起きる。
- NativeManager::detachはledger.changeとprivate detaching gateだけで、pool.close/retain/getへ再入しない。Ledger::request_closeはclosing記録とweak-pool upgradeのlockをそれぞれ解放してからclose/retainする。Ledger::failもrecords lockを離してrequest_closeする。現sourceにledger→slotsとslots→ledgerを同時保持する逆転経路は見えない。
- 注意として、detachのprivate gateは同期blockし得る。gateを使うfixtureではrelease前のcloseを同executor上で待つ配置を避ける必要がある。既存対象fixtureはgate解放後にcloseし、新回帰はdefault seams。このgateを一般callbackのreentrancy安全性の保証に広げない。将来detachからrequest_closeへ再入させる変更はretain中のslots lockと衝突する。
- retainはidle queueだけを触り、active Object/Txやnative BeginRequest内CheckoutOwnerを取り出せない。close後のlate returnはmax_size=0/size>0でstock detachに進む。未完了native cleanupのcheckoutを早期返却する修正ではない。
- WorkerHandle Dropはsenderを手放して閉鎖を要求するだけ。native JoinHandleは独立observerに残り、native close→actual join→cause公開→Ledger.completed/live除去の既存順を維持する。returned/detached/handle_dropsをjoin証拠へ昇格していない。
- request_closeは同期部分で閉鎖とsweepを完了してからclose Futureの待機へ進む。後のFuture取消・CloseTimeoutでもclosingは残り、再closeでactual joinを観測できる。active Txの終了をawaitで待つ契約、原因の優先順位、登録失敗counterは変更していない。新queue owner/observer/strong cycleも追加していない。

## fixtureの最終再確認

初期a488 fixtureはfirst_pollが予期せずReadyの場合に完了Futureを後で再pollしていた。f6bcはwaiting_pendingの場合だけmulti_watchで再pollし、この指摘は解消済み。Ready OkならTx rollbackを試み、Pending後の異常な成功Txにもrollbackを試みる。waiter/Adapter Drop→observer actual join観測の後にassertする順は維持された。future panicは既存multi_watchのcatch_unwindで記録できるが、それに頼って不要な再pollを残してはいない。

one-worker観測、closed主oracle、cleanup後assertは削除・緩和されていない。Config::defaultのin-memory fixtureで新しいdirectory削除は無い。watchdogは失敗検出であり、CloseTimeout成功化や再試行でoracleを隠していない。最終f6bcの全回帰結果はrootの記録を待つ。

## allocationと保証範囲

retainは `Vec::with_capacity(self.status().size)` を確保する。このsizeはactive checkoutも含むため、idleが0でも非zero capacity allocationが起こり得る。constant falseの掃除はidle件数を処理するが、一時容量はsnapshot sizeに依存し、再closeでも確保し得る。alloc-free、固定Future/全処理コスト不変とは報告しない。WorkerHandleの移動とDropであり、payload cloneや新依存/default/公開capを加える修正ではない。

`runtime/src/lib.rs:13`の#[cfg(test)] sqlite_prototype内だけの修正で、公開Pool/Txの実装や本番runtimeの終了保証追加ではない。巨大capacityのstock allocationブロッカーを解消したことにもならない。#87のmove仕様/Copy政策/生成責務、S1 Task設計をこのprivate修正へ広げない。
