# SQLite公開縦切りの前に検証すること

2026-10-06。[一接続adapterの結果](sqlite-adapter-results.md)を踏まえた次の設計メモ。ここにある公開配線の案は未実装。native容量と独立joinのprivate縦切りは[内部設計](sqlite-multiconnection-design.md)を先に記録し、[先行回帰の結果](sqlite-multiconnection-results.md)と分ける。取得期限・巨大capacityは読み取りレビューから得た候補で、実行再現済みとは扱わない。Q002／Q004の公開API・所有契約・policy・依存は維持する。

## native容量と独立した終了観測

次の候補は、starting／healthy／取消済み／detachedをすべてlive native recordに含め、`live.len() < connections`確認と登録を同ledger lockで行うこと。cause公開とactual join後にだけrecordを除く。slot選択・待機順・公平性はstock poolへ任せる。これは既存一接続の条件を数字だけ変えれば完成するという案ではない。

#82の[reaper](https://github.com/disnana/Nagi/blob/7999bab40b0a85b23ddf13b230e9e2db2c7ac3c9/runtime/src/sqlite_prototype/adapter.rs)は受信した最初のJoinHandleをblocking joinする。一接続の比較では前workerの終了待ちが必要だが、多接続ではhealthy Aが残る間、終了済みBの観測もAの後ろへ止まる。容量だけを接続した中間source `3a9824d`でこの反例を観測した。先行試験は、cap2でA activeを保ち、B startup取消→native exit→actual join→C取得をA終端前にassertする。filesystem DBを使い、公開契約が拒否する複数`:memory:`とは分ける。

private縦切りではworkerごとの独立join observerを採用する。native worker＋observerの約2thread/connectionとなり、費用は同条件で観測する。observerの起動失敗でnative JoinHandleをdetachしない所有順も先に検証する。timer polling、独自公平性algorithm、Tokio runtimeの全面交換は加えない。これだけで公開Poolのacceptanceとはしない。

## 取得期限の対象

Q002の`acquire_ms`はslot予約待ちで、SQL／BEGIN／busy／返信待ちのdeadlineではない。deadpool 0.13.1の`Timeouts.wait`はlogical permitだけを対象にする。`create` timeoutはnative開始全体まで及び、`recycle` timeoutはget loopで破棄・補充へ進むので、そのまま同じmsを設定する案は採らない。段階ごとに予算をリセットする案、begin全体をtimeoutで包む案も契約と一致しない。

native容量の解放待ちを予約条件へ含める場合は、logical待ちから同じ絶対予算を引き継ぎ、record登録で予約完了とする案を検証する。Manager.createには取得ごとのbudget引数がない。共有mutableな「現在の期限」は並行取得で成立しない。Tokio task-local scopeは候補だが、明示budgetとの対応、0ms、同taskの複数取得、取消時の寿命を確認するまで採用しない。eager prefillはopenの失敗・取消・資源確保の時点を変えるため、単にdeadline問題を避ける口実として追加しない。

先行oracleは、空きslotの0ms成功、logical/native容量不足の0ms失敗、有限予算の引継ぎ、登録後のBEGIN/busyが取得期限の対象にならないこと。deadlineやeager/lazyに新しい利用者契約が必要ならStopし、理由と代替案を示す。

## 巨大capacityと既存crateの境界

stock deadpoolのbuilderは`VecDeque::with_capacity(max_size)`を実行する。`max_size(0)`からresizeする案も`reserve_exact(additional)`を行うため、全capacity分のinfallible allocationを避けられない。正数・usize・Semaphore上限の検査だけでstock配列のbyte容量が可表現とは保証できない。これはsource読取で確認した条件で、巨大allocationを実際に試した結果ではない。

Q002の「巨大値を無条件に確保してvalidation済みとしない」を満たす可表現性・allocation失敗の扱いを整理する。任意の数値上限、fork、新依存を勝手に追加しない。Rustのglobal OOM／abortを普遍的に回復する保証も作らない。公開APIに数値制限や依存変更が必要になれば、その判断案を先に提示する。

## 小さい縦切りの順序

1. 上のnative cap・終了観測・予約期限についてfailing testsを作る。stock detach-entry、close／failedと登録、A active中のB panic／close失敗もpositive barrierで確認する。
2. 公開runtimeとOptions／Parameters／Failureを成立させ、旧Dbを維持する。新APIに不足するpolicy判断があれば実装を止める。
3. canonical registryと[compiler境界](sqlite-compiler-boundary.md)の実payload／capture／sealed SQLを同時に接続する。予定46入力をparseだけでなくpositive／negative・元位置・Rust buildへ昇格する。
4. High／保存Low／手書きLow、実DB、4 OS、生成探索、縦切りのcostを検査してG-TX／G-POOLとPhase 4 acceptanceを判断する。Phase 5はその後。

一次根拠: [deadpool Manager](https://docs.rs/deadpool/0.13.1/deadpool/managed/trait.Manager.html)、[Timeouts](https://docs.rs/deadpool/0.13.1/deadpool/managed/struct.Timeouts.html)、[pool.rs](https://docs.rs/crate/deadpool/0.13.1/source/src/managed/pool.rs)、[Q002のAPIと0ms](sqlite-pool-proposal.md)、[ADR 010](adr/010-sqlite-transaction-boundary.md)。仕様・採用判断と、sourceから考えた未検証案を分けて記録する。

## 取得期限候補の独立sourceレビュー

2026-10-06。Solによるdeadpool 0.13.1とTokio 1.53.1のsource照合では、取得ごとのtask-local scopeで予算を渡す候補はprivate試験へ進められる。deadpoolのcreate/recycleはcaller future内でpollされ、Tokio scopeはpoll/Dropの前後で値を復元する。同じtaskのjoin!/select!でもscopeを個別にする。別task/native threadへ予算を伝播させず、共有mutableな現在期限は使わない。これは読み取り結果で、実装・実行検証はまだない。

候補はImmediateまたは絶対deadlineを保持し、stock waitへ残予算、create/recycle timeoutはNoneとする。native登録で取得期限を終え、ready/BEGIN/busyへtimerを持ち越さない。backendのAcquireTimeoutとstock Wait timeoutの種別を保持し、期限切れだけでPoolをfailedにしない。recycle失敗は現在のretirement/failureを維持し、予算を作り直してreplacementを許可しない。

先行試験は0msの空き/論理不足/native不足、logical→nativeの残予算、同taskの異なる2予算、取消、登録後startupを期限超過させて解放後成功、recycle故障を対象にする。stock timeout_getは相対Durationからtimerを作り、Tokio TimeoutはReadyをtimerより先に返す。非yield/executor遅延も含む厳密な壁時計上限や、期限後のReadyを必ず拒否する新保証は加えない。

巨大capacityは、正数/usize/Semaphoreと時間変換に加え、WorkerHandleを含む配列が確実にbyte表現不能な入力をallocation前に拒否する検証は可能。ただしその下限検査だけでprivate ObjectInnerの全容量を検証したことにはならない。公開Objectのsizeofを代用した余分に厳しい上限や、試しの巨大allocationは採らない。stock infallible allocationの扱いと公開受理範囲が残り、新policy/依存変更が必要なら判断へ戻す。
