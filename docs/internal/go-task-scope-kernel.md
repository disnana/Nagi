# Go private Task scope: normal-success kernel

状態: **内部実装・公開未接続**。基点はmain `012f856a`。Nagi 0.3.0の
公開Go Task/scopeを部分的に成功扱いせず、採用済み
[`Task`契約](adr/012-task-result-handles.md)のうち設計判断を増やさない
正常終了境界だけを`compiler/go-runtime/internal/taskscope`で実行する。

## この段階で固定すること

- Spawnはreceiptとscope ticketを先に登録し、workを新しいgoroutineで一度だけ実行する。
- Receiveは対象childのwork、defer、明示cleanupが戻った後に値を一回だけ渡す。
  無関係な兄弟の終了は待たない。Result相当の値も平坦化しない。
- Discardは出力参照だけを放棄する。childをcancel/detachせず、Finishのjoin責任を残す。
- Handleのcopyは同じreceipt stateを共有する。wrong scope、zero/forged handle、二重消費を
  private runtimeでfail closedにする。公開checkerのmove/escape保証を代替しない。
- Finishは新規Spawnを封印し、登録済みchildのworkとcleanupが全て戻るまで待つ。
  公開checkerが防ぐ未消費resultをprivate contract errorとして拒否する。
- completion channel close以後にuser callbackやcleanupを行わない。Goには物理goroutineを
  joinするAPIがないため、完了はcallback、defer、cleanupが戻った証拠であり、schedulerが
  goroutineを破棄した証拠とは表現しない。

## まだ実装しないこと

panic/Goexitは成功値にせずprivate failureへ閉じるだけで、公開`TaskFailure`ではない。
sticky primary/related cause、兄弟cancel、body Err優先、親cancel、deadline、同期owner
teardown、resource close、rollbackは次の段階で`ScopeExitPlan`との対応を設計・検証する。
公開High/LowのScope/SpawnBind/Task、Go生成applicationへのruntime組込みは引き続き
toolchain起動前に拒否する。

## 検証境界

barrier付きnative Go testで、並行開始、対象childだけの受取、cleanup前の受取禁止、
discard前後、全child Finish、handle copy競合、wrong scope/forged handle、panic/cleanup
panic/Goexitを確認する。sleep、GC/finalizer、goroutine countをcleanup証拠に使わない。
既存Rust integrationの1入口はGo JSON testのpackage/test完全在庫を照合し、skip、fail、
zero、重複、欠落を成功にしない。公開scope拒否のHigh/手書きLow回帰を維持する。
