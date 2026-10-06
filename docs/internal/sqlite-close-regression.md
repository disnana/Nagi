# SQLite比較試作: closeとidle返却の回帰

2026-10-06。[PR #87](https://github.com/disnana/Nagi/pull/87)のmove補強後、head `68d215b`の[Linux CI](https://github.com/disnana/Nagi/actions/runs/37454227747)で、取得予算の既存テストが最後のcloseに失敗した。runtime 188成功・1失敗。取得・SQL・rollbackの検査は成功し、`CloseTimeout: adapter workers not joined`になった。

対象の`sqlite_prototype`は[runtime/src/lib.rs](../../runtime/src/lib.rs)で`cfg(test)`に限定した比較試作。公開Dbや本番runtimeで同じ障害を再現した記録ではない。分類は試作の終了契約に対するP1。再実行で成功する場合も、終了確認の期限を延ばしたり失敗をskipしたりして解決扱いにしない。

## 原因と決定的な先行テスト

deadpool 0.13.1の`return_object`はidle queueへの挿入後、slots lockを解放してsemaphoreへpermitを返す。`close`は`resize(0)`してからsemaphoreを閉じる。`resize`はpermitを取得できなければidle queueの除去を止める。このため、idle Objectがあるのにpermitを取得できない状態では、closed poolがworkerのsenderを保持し、native workerが終了しない。閉鎖後の再resizeもno-opとなる。

CIの正確なthread interleavingはログからは分からない。ローカルのruntime全189件は再確認1回・探索6回とも成功した。一方、次のstock APIだけのfixtureで同じ終了契約の反例を決定的に作れた。CIでpermit返却の隙間が起きた、とまで断定しない。

1. 容量1のadapterでcheckoutを保持する。
2. 別の取得Futureを1回pollし、logical semaphore待ちのPendingにする。
3. checkoutを返す。Objectはidle queueへ入り、permitは公平semaphoreから待機Futureへ予約される。
4. 待機Futureを再pollせず、closeする。idle Objectはqueueに残るが、resizeは予約済みpermitを取得できない。
5. closeの結果を保持し、待機FutureとPoolをDropし、observerでactual joinを確認してからassertする。

[回帰テスト](../../runtime/src/sqlite_prototype/adapter_tests.rs)の`close_retires_idle_worker_while_stock_permit_is_reserved_for_unpolled_waiter`は、testのみのcommit `0051f7a`でexit101、2秒後のCloseTimeoutになった。後片付けのactual join検査は通った。stock crate、公開API、既存の期限、期待は変更していない。

## 修正と保持した契約

[adapter](../../runtime/src/sqlite_prototype/adapter.rs)の`request_close`で、stockの`pool.close()`後に`retain(false)`から残るidle WorkerHandleを取り出し、ledger lock外でDropする。`max_size=0`の後の返却はstock detachへ進むため、新しいidle entryを足さない。

- active checkoutとtransactionを途中で返却しない。rollback・native close・actual joinの責任は維持する。
- close通知、handle Drop、worker終了、observerのjoinを同じeventと扱わない。
- Manager.detachはstockのslots lock中に呼ばれる。現callbackはledger counterとprivate gateだけで、poolへ再入しない。callbackにpool再入を足す変更は、このlock順を再確認する必要がある。
- 取り出したWorkerHandleのDropはpoolのslots lock外。record/pool弱参照mutexを保持したままclose/retainしない。
- retainはstockの`Vec::with_capacity(status.size)`を使う。active checkout数を含む一時allocationがあり、closeをalloc-freeとは説明しない。公開capacityの[別ブロッカー](sqlite-capacity-decision.md)を解決する変更ではない。

`a488348`の先行GREENは0秒で成功し、native_closed=joined=1、pending_workers=0を待機Futureの再pollより前に確認した。全回帰もこのsnapshotで成功した。独立Solレビュー後、`f6bc74a`でfixtureの予期しないReady経路から完了Futureを再pollしないよう補強した。正常oracle、既存assert、worker cleanupの期待は据置。

最終`f6bc74a`で全回帰90 suite・900件、failed/ignored 0、fmt・全target clippyが成功した。試作の回帰1件を加えた数であり、前のmove補強時点の899件とは区別する。

RED/GREEN・最終全回帰・原CIログとsource/hashは[検証artifact](../../benchmarks/results/explicit-move-2026-10-06/readiness/sqlite-close/)へ保存する。更新後の4 OSと必須CIは、PRの最新headのChecksで確認する。過去のheadの成功を最新コードの成功と数えない。

一次根拠: [deadpool pool.rs](https://docs.rs/crate/deadpool/0.13.1/source/src/managed/pool.rs)、[object.rs](https://docs.rs/crate/deadpool/0.13.1/source/src/managed/object.rs)、[Tokio Semaphoreの公平性](https://docs.rs/tokio/1.53.1/tokio/sync/struct.Semaphore.html)。第三者crateや生成Rustは修正していない。
