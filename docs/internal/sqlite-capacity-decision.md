# SQLite公開capacity: stock allocationの残る判断

2026-10-06。[Q002](sqlite-pool-proposal.md)は数値default／任意の上限を加えず、正数・native変換・Tokio受理範囲を検査し、巨大値を無条件に確保してvalidation済みとしない方針。Q004はdeadpool 0.13.1の比較試作を承認した。公開Poolへそのまま配線してよいとするacceptanceではない。

## sourceから確認した境界

stock builderは`VecDeque::with_capacity(max_size)`で全slot容量をinfallibleに確保する。0でbuildしてからresizeしても、`reserve_exact`による確保が残る。`ObjectInner`はprivateで、wrapperからその配列のLayoutやfallible予約へアクセスするAPIはない。

正数、usize、Semaphore上限、時間変換だけでは、内部配列のbyte容量が可表現で確保に失敗しないとは言えない。`Layout::array::<WorkerHandle>`の下限検査は確実に不正な値を拒否できるが、実際のslot全体のLayout検査を置き換えない。公開Objectのsizeofを代理にすると、別の余分な制限を作る。

ここでは巨大allocationを実行していない。global OOM／abortの普遍的回復も保証しない。既存公開Dbの再現済み不具合として数えず、Phase 4公開配線前のP2設計ブロッカーとして残す。未解決のまま公開してQ002のvalidation契約に反する場合は、公開契約違反として別に評価する。

## 選択肢

| 案 | できること | 残る条件 |
|---|---|---|
| 現stock版でprivate小容量の検証を継続 | native容量／join／取得予算の契約を実行検証できる | 公開capacityの受理範囲とallocationの検査は未完了 |
| deadpool内でLayout検査とfallible予約を行うbuild APIを利用 | wrapperからprivate配置を複製せず、crate自身へ容量と確保を任せられる | そのAPIを備える版は本調査で確認できていない。上流対応・依存版変更・failure分類の判断が必要 |
| wrapperを再比較する | 同じlifecycle oracleを維持し、allocation APIも比較できる | 代替をまだ選定していない。依存の承認、4 OS、取得／取消／終了の再検証が必要 |

今は一案目で取得予算の縦切りを進める。公開化前に残る選択を判断し、その時点で必要な依存差分・移行・検証条件を提示する。source読取だけで二案目のAPIが存在するとは書かない。

## 採らない回避策

`catch_unwind`はallocator abortを回復しない。先に大きいbufferを`try_reserve`して捨てても、stockが次に行う確保の成功を証明しない。private slot構造のコピーや任意の数値capを承認なしで追加しない。巨大値を試してプロセスを落とすこともvalidationの代わりにしない。

次の判断では、公開Options／openのどこで容量・確保失敗を返すか、Failure分類、受理範囲を明示する。公開配線前に未決のまま成功扱いすることを避ける。[ADR 010](adr/010-sqlite-transaction-boundary.md)と[公開縦切り計画](sqlite-public-slice-plan.md)を参照。

一次根拠: [deadpool pool.rs](https://docs.rs/crate/deadpool/0.13.1/source/src/managed/pool.rs)、[object.rs](https://docs.rs/crate/deadpool/0.13.1/source/src/managed/object.rs)、[Rust Layout::array](https://doc.rust-lang.org/std/alloc/struct.Layout.html#method.array)、[VecDeque::try_reserve_exact](https://doc.rust-lang.org/std/collections/struct.VecDeque.html#method.try_reserve_exact)。Solによる独立sourceレビューと照合した。allocation結果を測定した資料ではない。
