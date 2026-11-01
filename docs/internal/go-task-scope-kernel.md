# Go private Task scope: completion and cooperative-drain kernel

状態: **内部実装・公開未接続**。正常終了kernelはmain `f2cc1a46`までに統合済みで、
このfault/cancel/drain拡張はローカル検証中。Nagi 0.3.0の
公開Go Task/scopeを部分的に成功扱いせず、採用済み
[`Task`契約](adr/012-task-result-handles.md)のうち設計判断を増やさない
completion/fault/cancel/drain境界を`compiler/go-runtime/internal/taskscope`で実行する。

## この段階で固定すること

- Spawnはreceiptとscope ticketを先に登録し、workを新しいgoroutineで一度だけ実行する。
- Receiveは対象childのwork、defer、明示cleanupが戻った後に値を一回だけ渡す。
  sticky scope faultがないhealthy経路では無関係な兄弟の終了を待たない。fault経路は
  全childをdrainしてから返す。Result相当の値も平坦化しない。
- Discardは出力参照だけを放棄する。childをcancel/detachせず、Finishのjoin責任を残す。
- Handleのcopyは同じreceipt stateを共有する。wrong scope、zero/forged handle、二重消費を
  private runtimeでfail closedにする。公開checkerのmove/escape保証を代替しない。
- Finishは新規Spawnを封印し、登録済みchildのworkとcleanupが全て戻るまで待つ。
  公開checkerが防ぐ未消費resultをprivate contract errorとして拒否する。
- completion channel close以後にuser callbackやcleanupを行わない。Goには物理goroutineを
  joinするAPIがないため、完了はcallback、defer、cleanupが戻った証拠であり、schedulerが
  goroutineを破棄した証拠とは表現しない。
- callbackまたはcleanupのpanic/Goexitは成功値にしない。completionの公開順で最初に
  観測したfaultをsticky primaryとし、未完了の兄弟へ`context.CancelFunc`を一度要求して、
  全childのcallback/cleanup境界が完了するまでdrainする。cancel要求はjoin完了ではない。
- callbackが通常returnした値は、`context.Canceled`を含むbusiness valueでもfaultへ
  再分類しない。Result相当値も平坦化しない。
- Discardは出力だけを放棄し、未観測faultを捨てない。Cancelは新規Spawnを封印して
  cooperative cancelとdrainを行うが、値を消費済みとは偽装しない。
- 先行するscope primaryがないchildでcallbackとcleanupの両方が失敗した場合はcallback
  faultをprimary、cleanup faultをrelated causeとして保持する。既にprimaryがあれば
  両方をrelated causeへ追加する。任意のpanic payloadはformat・保持しない。

## まだ実装しないこと

private faultは公開`TaskFailure`ではない。body Err優先、legacy task faultとの統合、
親cancel/deadline、同期owner teardown、resource close、rollbackは次の段階で
`ScopeExitPlan`との対応を設計・検証する。
公開High/LowのScope/SpawnBind/Task、Go生成applicationへのruntime組込みは引き続き
toolchain起動前に拒否する。

## 検証境界

barrier付きnative Go testで、並行開始、対象childだけの受取、cleanup前の受取禁止、
discard前後、全child Finish、handle copy競合、wrong scope/forged handle、panic/cleanup
panic/Goexit、fault観測順、兄弟cancel、cancel後panic、非協調childのdrain、
Receive/Discard競合、Cancel/Finish競合を確認する。sleep、GC/finalizer、goroutine countを
cleanup証拠に使わない。
既存Rust integrationの1入口はGo JSON testのpackage/test完全在庫を照合し、skip、fail、
zero、重複、欠落を成功にしない。公開scope拒否のHigh/手書きLow回帰を維持する。
