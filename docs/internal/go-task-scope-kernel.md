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
- `RequestAbnormalExit`はscopeの封印、未claim出力の放棄、この呼び出しが担当する
  取消関数の取得を同じlock区間で行う。取消関数はlock外で実行し、cleanupや完了記録は
  待たない。別の呼び出しが先に取得した取消関数の実行順は変えない。完了記録の責任は
  残り、`FinishBodyError`はこの操作の後でdrainする。

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

### 公開型へ接続する位置

将来の生成コードは、`Receive` の値 `T` と fault を別々に扱う。fault が空なら
`T` 全体を外側の `Ok` に入れる。`T` の業務 `Err` に内部 fault と同じ値が
含まれても分類しない。受取の fault が `ErrTaskPanicked` なら、
Rust と同じ `TaskFailureKind.Panicked`、メッセージ `child panicked` へ対応付ける。
正常な scope 終了経路で `Finish` が同じ primary を返した場合は、同じメッセージを
持つ `Internal` の `Error` に変換する。この二つの変換は kernel が drain を
終えた後に行い、panic payload や Go のエラー文字列を分類に使わない。
body `Err` の経路は `FinishBodyError` で元の値を保持する。

現時点では公開 `TaskFailure` の Go 表現がなく、[Go emitter](../../compiler/src/go_backend.rs)
が生成する `nagiError` には `Invalid` と `Busy` だけが定義されている。
kernel と生成コードの共通変換入口はまだないため、この段階では内部の値と fault の
識別をテストする。`ErrTaskGoexit`、cleanup 固有の fault、protocol fault の
公開分類はこの変換から隔離する。`Goexit` は Rust の `Cancelled` へ割り当てない。

非公開の生成テストでは、High から検査済み Low を経由して子の Go コードを
実際の emitter で生成する。Rust のテスト専用コードが受取・scope 終了の変換を
別ファイルに生成し、非公開 kernel と実行する。Go 側の検査は cleanup を保留して
受取の完了を観測し、正常値、内側の業務 `Err`、panic の両方の変換先を照合する。
通常の CLI に Task を解禁する変更は含まない。実行コマンドは次のとおり。

```sh
cargo test --locked -p nagic --lib generated_single_child_task_projects_panic_and_preserves_business_result -- --ignored --nocapture
```

別の非公開 emitter 入口では、checker が封印した scope 退出、Task spawn、Task
受取の各 plan を照合し、Nagi ソースの scope・spawn・await 自体を Go へ下げる。
対象は main の単一 scope、引数なしの子、直接の一回受取に限る。子の本体は
スカラー Result の定数 return と任意の真偽値 assertion のみ。通常終了の
`Finish` は先の生成テストと同じ内部変換を使用する。Go の観測用 hook は
cleanup を止めたり受取値を読み出したりするためだけに使う。

scope 本体には、受取の直前または直後に、リテラルの `error("...")` を扱う
`try` を一つだけ置ける。通常の `try` lowering は字句的な本体を退出し、
その後で `FinishBodyError` が本体の未受取出力を退役させて子を drain する。
Rust の `TaskScope` と同様に、子の panic を受け取った後も本体の元の Err を返す。
受取前の試験では、子の cleanup をチャネルで保留し、取消要求を観測してから
解放する。一般の取消や実資源の解放はこの試験の範囲外である。

受取の代わりに、同じ単一子 handle を正規の `std.task.discard` に渡す形も
検証する。本体の `try` との組合せは対象外。sealed
`DiscardOutputKeepScopeJoin` を照合して kernel の `Discard` を呼び、通常の
`Finish` まで進む。試験は子の処理を discard 後まで止め、取消が要求されて
いないことを確認してから子を進める。別の cleanup gate の解放後、正常値なら
scope は成功し、子 panic は scope の `Internal` に残る。暗黙 Drop は扱わない。

```sh
cargo test --locked -p nagic --lib checked_single_task_lowers_scope_spawn_and_await_internally -- --ignored --nocapture
```

### scope 本体の panic

生成 Rust では、scope 本体の unwind が未受取 handle の出力を放棄し、外側の
`TaskScope` の Drop が生存する子へ取消を要求する。元の panic は呼び出し側へ
伝わる。親 Future を保持したまま同じ executor を再駆動すると、取消された子の
cleanup を別に観測できる。High・保存 Low・手書き Low の生成テストでは、完了済みの
出力の Drop と、待機中の子の cleanup を別々に検証する。

たとえば、待機中の子の cleanup を止めた状態で本体が panic すると、`Cancel` や
`FinishBodyError` を defer した Go 側は drain で止まり、呼び出し側への panic の
伝播を遅らせる。test build 専用の単一子 Go lowering は、本体の直接の spawn と
await の間に組み込み `assert_true` の真偽値リテラルを一つ置く形に限り、
字句的な本体からの unwind を `RequestAbnormalExit` につなぐ。defer は未受取出力の
放棄と取消要求までを担い、元の panic は呼び出し側へ伝わる。

生成テストでは既存の観測用 hook で子を取消チャネルに待機させる。実際の kernel の
取消関数を観測し、出力放棄後に一度だけ呼ばれたこと、cleanup のゲートを閉じたまま
元の assertion panic が届くことを確認する。ゲートを開いた後、観測側が別途
`Finish` を呼び、子の完了の記録と entry の退役を確認する。`assert_true(true)` は
通常の受取と `Finish` を通る。

単一子の受取経路には、0 以上の i32 に収まるミリ秒リテラルの `await sleep` を
既存のスカラー Result の直前に一つ置く形もある。この spawn は子の checked body
から取消を受け取る cold callback を生成する。実タイマーと `ctx.Done()` を
select し、どちらで戻ってもタイマーを停止する。取消を選んだ場合は残りの子本体を
実行せず、業務値を公開しない。要求済み取消の cleanup は完了記録まで進められ、
cleanup 自体の故障は scope に残る。本体 Err は drain を待って元の Err を返す。

タイマー生成と取消分岐のテスト hook は通知だけを行う。呼び出し側が panic を
受け取る時点と、cleanup を解放して子を回収する時点は別々に観測する。
通常の cold Future と `sleep` は既存の生成経路を使う。この限定経路では
取消済み handle の公開分類、一般の待機や I/O、公開 CLI への接続は扱わない。

### 論理 join として使うための条件

`done` は scope の完了登録より先に観測できる。テストでは scope の lock を
保持したまま子を終了させ、receipt の通知が来ても完了が未登録・未記録である
状態を確認する。公開接続で確認すべき境界は、子の Nagi 処理と必要な cleanup が
すべて完了し、その完了を scope が記録した時点である。単一の管理対象の子でこの
条件を満たし、受取後に子の処理や cleanup が残らなければ、待機側から見える
順序は Rust のテストと一致する。

子が登録外の goroutine を起動した場合や、cleanup が非同期 close を開始して
戻った場合は、同じ通知を観測しても処理や資源解放が残り得る。公開経路にその
入力を含める根拠はない。また scope が登録を記録した時点で、wrapper goroutine の
物理終了を証明できない。現在のテストは Go の非公開 kernel の境界を固定するもので、
生成 Nagi コードが必要な cleanup をすべて接続したという証拠はまだない。

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
