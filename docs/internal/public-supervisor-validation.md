# 公開Nagi Supervisorの有限回帰と計測（2026-10-27）

対象baseは `0d1884cb5a24235a440811a9f5d16be1bf5a664a`、Rust backendの既存0.2.1 API。
compiler/runtimeのproduction code、既定設定、依存、言語契約、CI workflowを変えない。
#183の直接Tokio taskによる合成probeとは別に、公開`std.actor`をNagiから実行する。
[再現手順](../../benchmarks/public-supervisor/README.md)と
[固定した原計測・hash](../../benchmarks/results/public-supervisor-2026-10-27/README.md)を参照。

## 実際に通した境界

`compiler/tests/fixtures/public-supervisor/contract.nagi` と手書き`contract.low`を既存
`native_triple`へ登録した。Highをparse/check、生成保存Lowを元High削除後にも独立check、
手書きLowをparse/checkし、各々の未編集emissionを実production runtimeへCargo build・native実行する。
公開registryの`ActorRegister`等は`compiler/src/stdlib.rs`で`::nagi_runtime::actor::*`へ結び付く。
新しいactor/mailbox/supervisor実装、直接Tokio spawnの代用品は追加していない。

Rust adapterは故障注入、有限Notify待ち、guard/context/state Drop数の観測だけを担当する。
NagiがSupervisorを構築・登録・run・call・event監視・shutdownし、次をassertする。

| ケース | 公開APIと観測する結果 |
| --- | --- |
| 再起動予算0/2、連続worker Err | 最初の実行+予算分だけ起動、再起動数が予算と一致、INTENSITY_EXCEEDEDが1、runはErr、兄弟も停止 |
| panic、予算2 | PANICKEDが3、再起動2、Errと区別。unwind後に全guard破棄 |
| PERMANENTの正常終了、予算2 | FAILED/PANICKEDは0でも再起動2、最後は予算超過で停止 |
| 業務Errとworker Err | 業務Err(99)は返信となり状態12を維持、worker ErrはREPLY_LOST、公開generation2後に初期状態7へ戻る |
| 兄弟への影響 | 予算超過前は兄弟1世代。片方のworker故障後も別actorの状態10とgeneration1を維持 |
| 明示shutdown | PERMANENTでも再起動せず、pending childのguardとcontextを破棄 |
| 親scope取消 | 実Nagi scopeのErrでactor.run所有者を取消、公開Control.shutdownで完了を待ち、再起動0・全guard/context破棄 |

全ケースでactive=0、起動回数=破棄回数、同一世代の重複activeなしを確認する。
状態ケースはfactory3回、状態Drop5回、context Drop1回をassertする。
Context/Stateに所有strを置いてCopy型とせず、試験adapterでDropを観測する。
初版のprimitive-only fixtureにDropを追加した際のRust E0184はfixture不備として修正し、
compiler側のCopy/ownership契約やassertを弱めていない。

公開の専用cancel APIやtask/ledger snapshotはない。親scopeの取消と公開shutdownを使い、
private test snapshotをNagi APIと誤認しない。shutdownのErrだけでは「記録済み故障を伴う
cleanup完了」と「期限内にcleanup未完了」を区別できないため、event終了とDropも照合する。
既存actor_stdlib/runtime actor testsが持つadmission・timeout等の網羅をこの新1testの実績と
混同しない。今回新回帰のnative実行targetはLinux x86_64であり、4OS新native成功は未確認。

## 有限benchmark

全登録・factory・handler・call・再起動確認・停止は`benchmarks/public-supervisor/main.nagi`。
Rust adapterは時刻、返信の成否、公開Tokio task metricを記録するだけでspawnしない。
Python controllerは`/proc` RSS/FD/threadを観測し、有限guardと所有processの回収を担当する。
全プログラムはlocalhost socketすら使わないprocess内部通信で、外部/本番負荷はない。

1/2 logical CPU affinity（予約vCPUではない）、NAGI_THREADS=1/2、actor数1/32/128/512、
各3round、各3phase、2送信者×20,000完了を事前固定した。1actorだけは両送信者が同一actorを使い、
故障なし。3actor以上は別々の正常actor2つと故障対象1つ、残りはready・idle。
公開callの`Ok(Ok(reply))`だけを成功として数え、意図したworker故障は別にREPLY_LOSTをassertする。
rate limiterはなく、mailbox 64・入力/返信4KiBは8-byte messageに対して十分余裕がある。
これは2 outstanding callのclosed-loopであり、mailbox飽和・最大throughput・HTTP testではない。

24process / 72phase、合計2,880,000正常返信、正常呼出の拒否/timeout/業務Err=0、
18回の故障は公開generation2と状態0への再初期化まで確認した。
全72phaseでCPU時計はpositive。短いsmokeで0nsを観測したため、本計測は20,000回へ延長し、
集計は0nsを欠測とする回帰を持つ。CPU時間は送信者・観測処理・故障処理を含むprocess全体。
controllerのCPUは別processであり、専用ホストを予約した測定ではない。

各process wall30秒、CPU15秒（hard16秒）、sampled RSS512MiB、空き8GiB下限で停止する。
1 GiB = 1,073,741,824 bytes。ビルドはshared target/jobs1、計測時は他Cargo/Go/Nagi作業を止めた。
ホストcgroup quotaは4CPU相当、許可affinityは5logical CPU。対象processのRSSとcontainer全体を区別する。
測定process累計wallは17.13秒であり、全体の作業時間やbuild時間ではない。

最終checkpointのFD差0・Tokio task差0を全24caseで確認した。中間checkpointではshutdown後も
driver自身が生存し、ハンドル放棄後のscope終了+固定50ms時点を最終観測にする。成功までpollする
回収待ちではない。RSSはallocatorのhigh-water・event storage・最後の40,000返信の観測vectorを
含んで残る。RSSが元へ戻らないことをactor leakと断定せず、RSSだけを回収の証明にも使わない。

Nagiの現契約は非Copy resourceのList view/indexを提供しないため、benchmarkは3本の明示handleと
残りのList消費反復を使う。これを性能理由でcompiler側から迂回していない。

## BEAMとの差と使い分け

`runtime/src/actor.rs::register`はRust所有state、Arc context、Tokio mpsc/semaphore/oneshotを使い、
`run`は一個のSupervisorに共通のsliding-window再起動予算を持つ。一子の故障は予算内なら
one-for-oneで、予算超過時は全体停止する。`runtime/src/actor/lifecycle.rs`がJoinHandle/abort/
guard/context破棄の完了を所有する。`runtime/src/lib.rs::block_on`はTokio multi-thread runtime。
Nagi actorは独立したOS processやBEAM process heapではない。同じaddress spaceとallocatorを使い、
Rust所有権・Dropを使う。per-actor tracing GCや総heap memory budgetはない。
mailboxの件数/byte boundはC、S、native保持、全processのメモリを制限しない。

[公式ERTS GC説明](https://www.erlang.org/doc/apps/erts/garbagecollection.html)はprocessごとの
stack/heapとgenerational copying GCを記述する。
[公式Erlang efficiency guide](https://www.erlang.org/doc/system/eff_guide_processes.html)は
process間messageのcopy（同一nodeのrefc binary/literalを除く）を説明する。2026-10-27に原文を取得した。
これはBEAMで全メモリやnative故障が完全隔離されるとの主張ではなく、今回はBEAMを比較実行していない。

| 故障・仕事 | 今回の保証と限界 |
| --- | --- |
| 業務Err | replyの失敗。worker再起動とは別 |
| 協調asyncのworker Err / unwinding panic | 今回実APIで回収・予算内再起動を確認。副作用のrollbackではない |
| process abort / OOM / native crash | unwinding panicとは別。Supervisorで復旧できる保証なし。試験しない |
| yieldしないCPU / blocking native / blocking Drop | Tokio取消・deadlineがpoll境界へ到達できず、他actorのCPUを奪い得る。強制preemptionの保証なし |
| 大きなstate・共有context・native allocation | mailbox制限の外。process/cgroup等の外側隔離が必要な場合がある |
| 独立した短いI/O操作 | 通常async/scopeで十分な場合がある。actor化が自動で速くなると主張しない |
| 所有stateと直列message処理・監視が必要 | actor/Supervisorを使い、予算・admission・shutdownを設計する |

独自scheduler/ractor/Kameo/Glommio、process隔離、memory budgetの大変更は提案に留める。
まずserialization、mailbox/permit、allocation、runtime scheduling、同期観測overheadを分離する
profilingと、有限pipeline数/handler負荷を変えた試験が次の段階。今回の2送信者や短い観測から
一般的公平性・高負荷耐久性・最大actor数・1GiB収容数へ外挿しない。

Go HTTP/actor未対応へRustの結果を転用しない。共通fixtureの期待値を基に、Go側のasync/scope・
所有権と取消・reply Error分類を先に定め、次にbounded mailbox/worker、最後にSupervisor再起動予算・
状態factory・cleanup完了のbackend契約へ進む。対応するまでcapability拒否を維持し、各stageを
実Go codegen/nativeで検証する。これは今後の依存順の計画であり、このPRのGo実装ではない。
