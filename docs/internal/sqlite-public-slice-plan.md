# SQLite公開縦切りの前に検証すること

2026-10-06。[一接続adapterの結果](sqlite-adapter-results.md)を踏まえた次の設計メモ。ここにある拡張案は未採用・未実装で、読み取りレビューから得た反例候補を実行再現したとは扱わない。Q002／Q004の公開API・所有契約・policy・依存は維持する。

## native容量と独立した終了観測

次の候補は、starting／healthy／取消済み／detachedをすべてlive native recordに含め、`live.len() < connections`確認と登録を同ledger lockで行うこと。cause公開とactual join後にだけrecordを除く。slot選択・待機順・公平性はstock poolへ任せる。これは既存一接続の条件を数字だけ変えれば完成するという案ではない。

現在の[reaper](../../runtime/src/sqlite_prototype/adapter.rs)は受信した最初のJoinHandleをblocking joinする。一接続の比較では前workerの終了待ちが必要だが、多接続ではhealthy Aが残る間、終了済みBの観測もAの後ろへ止まり得る。先行試験は、cap2でA activeを保ち、B startup取消→native exit→actual join→C取得をA終端前にassertする。filesystem DBを使い、公開契約が拒否する複数`:memory:`とは分ける。

単純な候補はworkerごとの独立join observer。native worker＋observerの約2thread/connectionとなり、費用は未測定。observerの起動失敗でnative JoinHandleをdetachしない所有順も先に検証する。timer polling、独自公平性algorithm、Tokio runtimeの全面交換は先に加えない。

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
