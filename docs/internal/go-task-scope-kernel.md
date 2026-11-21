# Go private Task scope: completion and cooperative-drain kernel

状態: **内部実装・公開未接続**。completion/fault/cancel/drainとbody error/owner
teardown境界はmain `ca848078`までに統合済みで、このbody-local output放棄拡張は
ローカル検証中。
Nagi 0.3.0の
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
- `FinishBodyError`は任意型`E`の元body errorをTask fault stateへ格納・formatせず保持する。
  未claimのbody-local Task出力を先に放棄し、owner teardownのcancel要求後、
  全callback/defer/cleanupをdrainしてから同じ値を返す。
  先に観測済みのchild faultやdrain中のchild faultは元body errorを置換しない。
- body-local出力の放棄は`receiptLive`だけを対象とする。先に`Receive`がclaimした出力を
  奪わず、完了済み値と後から完了する値のkernel参照を保持しない。scope封印と放棄を
  同じlock区間で行い、その間の新規Spawnを許さない。これは任意`T`の`Close`、
  finalizer、GCや物理memory回収を保証しない。
- `RequestOwnerTeardown`は新規Spawnを封印し、live childの`CancelFunc`を一度だけ取り出して
  lock外で実行する。同期呼出しはcleanup/joinを待たず、完了を報告しない。後続の
  `Cancel`/`FinishBodyError`が同じobligationをdrainできる。

## まだ実装しないこと

private faultは公開`TaskFailure`ではない。legacy task faultとの統合、親cancel/deadline、
一般owner/resource release、rollbackは未接続である。
今回のprivate関数は`ScopeExitPlan`の退出順序だけを検証し、公開loweringを有効化しない。
公開High/LowのScope/SpawnBind/Task、Go生成applicationへのruntime組込みは引き続き
toolchain起動前に拒否する。

## 待機側が観測する完了と故障

[ADR 012](adr/012-task-result-handles.md) の actual join を、Rust は
[`TaskScope`](../../runtime/src/task.rs) が所有する Tokio `JoinSet` の完了結果で確認する。
子の型付き値を送信できても、child Future の解放や join の記録が残っていれば
`receive` は待機する。join は Tokio タスクの終了を観測する。Tokio の実行を担う
ホストスレッドは存続し得る。同期 Drop は abort を要求するだけで、join 完了を返さない。

Go の非公開 kernel は、子の処理とその `defer`、明示 cleanup が戻った後に receipt の
`done` を閉じ、続いて scope に完了を登録する。`Receive` と `Finish` が使うのは
scope が記録した完了である。明示 cleanup が担当した資源解放はこの順序で検証できる。
任意の資源 close、GC、子が独自に起動した goroutine の終了、wrapper 自体の物理終了は
この通知から判定できない。

| 入力 | Rust の受取と scope 出口 | 現在の非公開 Go kernel |
|---|---|---|
| 子の値（内側の業務 `Err` を含む） | 受取は外側 `Ok(T)`。内側の `Result` を保持し、業務 `Err` 単独では scope を故障にしない | 型付きの値をそのまま返し、`Finish` は成功 |
| 子の panic | `TaskFailureKind.Panicked` と `child panicked`。scope は同じメッセージの `Error::internal` を返す | `ErrTaskPanicked` が `Receive` と `Finish` に残る。公開型への変換は未接続 |
| 予期しない native 取消、内部 protocol 故障 | それぞれ `Cancelled`、`Internal`。scope は同じメッセージの `Error::internal` を返す | 協調取消の要求や `Goexit` を Rust の取消へ対応付ける判断は未採用 |
| 旧 statement spawn の `Err` | `LegacyError`。scope は元の `Error` の kind と message を保持 | 旧 spawn の公開接続は未実装 |
| scope 本体の `Err` | body local を退役させ、abort と drain の後に元の値を返す | `FinishBodyError` が元の値を保持して cleanup まで待つ |

限定公開を検討する場合、既存契約が必要とする子の終了と資源の退役をすべて観測できる
入力だけに絞り、完了登録をその証拠として採用できるか判断する。その場合も panic の
公開 `TaskFailure` と sticky な scope `Error` はそれぞれ検査する。ネイティブタスク
自体の終了イベントが必要な場合は、同等の証拠を用意できるまで Go の公開拒否を維持する。
今回の内部テストはどちらの判断も採用せず、Rust の公開契約と ADR は維持する。

## 検証境界

barrier付きnative Go testで、並行開始、対象childだけの受取、cleanup前の受取禁止、
discard前後、全child Finish、handle copy競合、wrong scope/forged handle、panic/cleanup
panic/Goexit、fault観測順、兄弟cancel、cancel後panic、非協調childのdrain、
Receive/Discard競合、Cancel/Finish競合に加え、先に観測したchild faultよりbody errorを
優先すること、完了前後の未claim出力をcancel要求前に放棄すること、claim済みReceiveを
奪わないこと、放棄と並行するSpawnを拒否すること、owner teardownがcancel要求後も
cleanup完了を偽装しないことを確認する。
sleep、GC/finalizer、goroutine countをcleanup証拠に使わない。
既存Rust integrationの1入口はGo JSON testのpackage/test完全在庫を照合し、skip、fail、
zero、重複、欠落を成功にしない。公開scope拒否のHigh/手書きLow回帰を維持する。
