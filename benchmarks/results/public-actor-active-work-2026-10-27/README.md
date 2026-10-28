# 公開Actor active-work: 2026-10-27の有限実測

公開`std.actor` APIを使う実Nagiプログラムで、**全Actorが独立stateを更新する**処理を測定した。
2 clientだけが動く[前段の計測](../public-supervisor-2026-10-27/README.md)とはworkloadが異なる。
Rust/Tokioを直接spawnした代替actorではない。
[コード・上限・再現手順](../../public-actor-active-work/README.md)を参照。

18 process、合計**2,520,000成功返信、正常呼出失敗0**。4/16/64 active client、各5,000件、
CPU affinity1/2、各3roundを事前固定した。各clientは別のActorを公開registerし、
毎回reply=`i+1`をassertする。1Actorの取り違えやstate resetを正常完了として数えない。
各processに別の故障対象Actorが1つあり、意図したworker Errを1回送り、FAILED=1、
RESTART_SCHEDULED=1、STARTED generation2、factory初期状態0の返信を公開APIで確認した。
故障呼出のREPLY_LOSTは成功2,520,000件に含まない。retryはない。

AMD EPYC9V74仮想host、許可logical CPU0..4、host cgroup quota4CPU相当/memory16GiB。
targetのaffinityを1/2へ固定しNAGI_THREADSも1/2。専用vCPUを予約した結果ではない。
他のCargo/Go/native buildは計測中に実行しなかった。load client・計時vectorはtargetと同processで
CPU時間に含まれ、Python controllerは別process。短いinteger state/messageのmailbox基準値で、
I/O・HTTP・最大mailbox飽和や重い業務処理とは異なる。

| CPU affinity | active clients / actors計 | 成功/s 中央値 [min,max] | CPU ns/成功 中央値 | p50 µs | p95 µs | p99 µs | peak RSS MiB 最大 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 4 / 5 | 1,459,225 [1,130,193,1,608,021] | 670 | 3.345 | 3.956 | 8.733 | 7.95 |
| 1 | 16 / 17 | 1,395,042 [1,208,796,1,543,492] | 712 | 15.474 | 19.860 | 75.784 | 25.93 |
| 1 | 64 / 65 | 1,567,938 [1,250,421,1,628,898] | 632 | 65.058 | 92.460 | 184.889 | 97.79 |
| 2 | 4 / 5 | 1,764,544 [1,018,481,1,900,680] | 899 | 0.721 | 3.796 | 5.218 | 8.02 |
| 2 | 16 / 17 | 2,161,467 [2,135,049,2,284,850] | 872 | 4.998 | 13.841 | 44.728 | 26.10 |
| 2 | 64 / 65 | 2,414,785 [2,339,470,2,479,156] | 812 | 12.038 | 76.576 | 97.338 | 98.49 |

latencyは成功callの入場待機・mailbox・handler・reply受信を含むnearest-rank。
表は各caseで計算したquantileの3round中央値で、全roundを混ぜたquantileではない。
4clientのthroughputは既定10ms再起動待ちを含むphase全体のwallを分母にするため、
正常clientが先に終わった後の待機も含む。CPU/successは時計がpositiveの全18caseで計算した。
1→2CPUで速度は向上するが、CPU単価も増えており「並列化が無料」とは解釈しない。

| CPU affinity | active clients | 故障区間中の正常完了 中央値 | 故障区間 ms 中央値 | 同p99 µs 中央値 | ready RSS MiB 中央値 | scope終了50ms後 RSS MiB 中央値 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 4 | 16,330 | 10.516 | 10.766 | 1.906 | 4.629 |
| 1 | 16 | 11,682 | 10.517 | 124.578 | 1.945 | 12.445 |
| 1 | 64 | 14,922 | 10.883 | 259.132 | 2.164 | 64.715 |
| 2 | 4 | 20,000 | 10.493 | 5.218 | 1.957 | 4.672 |
| 2 | 16 | 19,592 | 10.362 | 92.470 | 2.004 | 12.020 |
| 2 | 64 | 26,034 | 11.132 | 163.146 | 2.195 | 42.309 |

故障区間は別Actorへの失敗call開始からpublic generation2・state0確認までで、
既定restart delayを含む。全caseでこの区間内にも正常Actorが実処理を完了した。
全18caseの最終FD差0・Tokio task差0。これをOOM/native crash隔離の保証としない。
**peak RSSには全reply timingのJSON化を含む観測allocationが入る**。
timing vectorは転送後にfreeするが、allocatorのpage retentionで最後のRSSは残る。
64active Actorに98MiBが必須だとも、RSSだけで全resource回収が証明されたとも言わない。

上限は各process wall30s、CPU15s(hard16s)、sampledRSS512MiB、disk空き8GiB。
最大観測RSS98.49MiBでguardに触れず終了した。1GiBという目標budgetの下で測る段階だが、
**hard1GiB cgroup/専用vCPU環境の検証ではなく、最大収容数の推計もしていない**。
phase wall中央値は約11〜204ms、process wall累計14.984s。長時間安定性の試験ではない。

## 保存・検証

- `summary.json`: 18caseのcheckpoint、CPU、FD/task、全phaseの集計
- `*.jsonl.gz`: 個々の全成功/失敗timingとpublic recovery記録
- `aggregate.json`: 各群のmedian/min/max
- `verification.json`: 18raw phaseを再集計して完全一致、2,520,000成功、18故障/再起動、raw SHA256
- `provenance.json`: source、compiler binary、runtime tree、生成Rust、native lock、binaryのhash
- `measured-*-runner.py.txt`: 実行したcontrollerそのもの
- provenanceのsaved Low hash: 最終Highから生成し、独立check済みのLowの同一性。Lowは再現コマンドで生成できる。手書きLow/4OSを実行したという証拠ではない

使用compilerは既存development `nagic0.2.1`、runtimeは#187 base0.2.1の公開Actor実装。
新しい0.2.2の性能比較・Go性能値としては扱わない。依存実解決はnative lockへ保存した。
initial-build4m35s後、故障event集計と観測vector解放を追加してwarm final-build3.52s。
native/High/保存Lowのcheck成功、accounting14tests成功、生成Rustのrelease実行成功。

初回smokeはActorの4,000返信と回復は通ったが、終了時の/proc fd読取がPermissionErrorになり
controllerが失敗扱いにした。process.pollでexitを確認できる場合だけ観測欠測とし、
live processのPermissionErrorは引き続き失敗にする回帰2件を加えた。
修正後smokeは成功し、smokeは本計測から除外。元smokeはlocal-onlyに保持する。

再集計だけを実行する場合:

```sh
python benchmarks/public-actor-active-work/verify_results.py \
  benchmarks/results/public-actor-active-work-2026-10-27
```

Go Actor/HTTP、Python/Elixir比較、インターネットDDoS、長時間soak、hard1GiB隔離は未測定。
公開・merge前の独立reviewと最終HEAD CIは別gate。
