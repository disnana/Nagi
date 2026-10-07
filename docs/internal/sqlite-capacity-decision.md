# SQLite公開capacityの判断

2026-10-08。stock deadpool 0.13.1の全slot infallible予約を公開へ接続するブロッカーは、既存Tokio Semaphoreとlazy専用adapterの採用によって解消する。ユーザーは正式化前の長期的な妥当性を優先し、必要な依存/API/Failure分類変更を承認した。最新release/mainにもfallible builderがないこと、限定vendorとbb8/mobcの費用は[一次調査](sqlite-capacity-research-2026-10-08.md)に残す。

## 受理範囲と責務

`options`はconnections/queue_capacityの正数・usize変換・`Semaphore::MAX_PERMITS`以下、非負ms・Durationとnative Instant加算、busy_msのi32変換を検査する。rusqlite busy_timeoutのi32変換panicを公開境界で防ぐ。任意の数値cap/default、無期限sentinelは加えない。

connections個のslot配列を持たないため、`Layout::array<ObjectInner>`という受理上限は適用しない。idleとnative ledgerは実際の接続数に応じて増え、追加の前に`try_reserve(1)`する。queue_capacityは各session inboxの上限であり全heapの上限ではない。Tokio bounded mpscも指定数のCommandを一括確保する構造ではない。

openは空path、URI、複数connectionの`:memory:`を拒否してpathを所有し、小さい管理構造を作る。native Connection/worker/observerはbegin時にlazy起動する。取得予算はSemaphore待ちからnative record登録まで同じ絶対期限で、登録後のopen/ready/BEGIN/busyは含まない。0msは条件が即時に満たされれば成功する。

## 予約失敗と終了責任

新`FailureKind.ALLOCATION`はfallible container予約が実際にErrを返した場合に用いる。設定不正のINVALID、worker/native起動や故障のWORKERと区別する。primary causeは既存Errorのkind/messageを保持し、明示copy operationで取得する。

native ledgerの予約失敗はworker起動前に`NOT_APPLICABLE / false`を返す。idle queue返却の予約失敗はDrop内なので、`NOT_APPLICABLE / true`をledgerへ記録してclosingへ進める。idleをlock外で破棄し、workerのsenderを解放し、独立observerが実joinとcause公開を終えるまでlive recordを除かない。返却処理を終えたことを回復やjoinの証明にしない。

最後のPool ownerのDropでもclose要求を開始する。checkout/TxはPoolを強参照せず、active Txのcleanupを完了させてからnative終了する。明示closeのtimeout/取消でclosingを解除しない。再closeで実完了を待てる。close/acquireのFailure outcomeはTxがないためNOT_APPLICABLEとする。

## 検証の限界

巨大allocation/resource exhaustion実験は行わない。小さいfault injectionで予約失敗と停止/joinを検査し、通常cap1/cap2とscalar境界で受理範囲を検査する。Arc/String/Tokio/allocator全体をfallible化せず、global OOM/abortの普遍回復は保証しない。SQL制限・native上限・独立join・取消・取得予算の既存oracleは公開本体上へ移す。compiler三構文/46入力と4 OSの完了はruntime単体成功と区別する。

採用理由・API差分・配布境界・検証ログは[公開runtime判断](sqlite-public-runtime-decision.md)、公開契約は[Q002](sqlite-pool-proposal.md)、決定履歴は[ADR 010](adr/010-sqlite-transaction-boundary.md)を参照。
