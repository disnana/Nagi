# 公開Nagi Actorのactive-work計測

`main.nagi`が公開`std.actor` APIで独立したActorを登録し、各Actorに1個のNagi clientが
整数の加算messageを送る。replyが各clientの`i + 1`と一致することを毎回assertするため、
同じActorを複数clientへ誤って共有した場合も通らない。Rust adapterは計時・結果保存と
公開Tokio task数の観測だけを行い、worker/client/Actor/Supervisorを実装・spawnしない。

これは[既存probe](../public-supervisor/README.md)の「2 active client + 多数idle actor」に続く
**4/16/64 active client + 同数の独立Actor + 故障対象1Actor**の有限試験である。
HTTP・最大throughput・1 GiBに入る最大Actor数・BEAMとの同等性を測る試験ではない。

各clientは5,000件、全件の`Ok(Ok(value))`完了だけを成功として数える。
retryはしない。拒否・timeout・業務Err・キューへの投入だけを成功には数えない。
mailboxは32件、入力/返信byte予算は各4,096 bytes、message/replyは各i64。
各clientは1 outstanding callで、mailbox飽和試験ではない。

正常clientと同時に別Actorへworker Errを1回送り、公開eventでFAILED=1、RESTART_SCHEDULED=1、
STARTED generation=2を確認し、状態0の返信でfactoryによる再初期化を確認する。
正常clientの進捗と完了数は故障区間の開始/終了時刻と照合する。
PANIC/OOM/abort/native crashの復旧保証をこれで主張しない。

## 実行

Linux64、runtime sourceと対応するcompiler、Rust toolchainが必要。
`NAGI_NATIVE_TARGET_DIR`は生成applicationのCargo cacheである。`CARGO_TARGET_DIR`だけでは
Nagiが選ぶnative cacheを指定したことにならない。compiler自身のcacheとは区別する。

```sh
export NAGI_NATIVE_TARGET_DIR=/absolute/shared-native-target CARGO_BUILD_JOBS=1
nagic check benchmarks/public-actor-active-work/main.nagi --no-project
nagic lower benchmarks/public-actor-active-work/main.nagi --no-project --out /absolute/new-low
nagic check /absolute/new-low/generated.low --no-project
nagic build benchmarks/public-actor-active-work/main.nagi --no-project --profile release \
  --rust benchmarks/public-actor-active-work/observe.rs --rust-dep tokio=1.48 \
  --out /absolute/owned-build
python scripts/public_actor_active_probe.py --binary /absolute/native-path-printed-by-build \
  --output /absolute/new-measurement --clients 4 16 64 --cpus 1 2 --iterations 5000 --rounds 3
```

出力先は新規でなければならない。1/2 CPU affinityを別々に測り、NAGI_THREADSも1/2に設定する。
affinityは予約vCPUや専用coreではなく、hostのcgroup quotaと他process負荷も記録する。
client、時計・保存処理はtargetと同processであり、CPU/successに含まれる。
Python controllerは別processで、JSON転送後の解析も計測区間の外にある。

## 上限と測定の定義

1 processはwall30秒、CPU15秒(hard16秒)、sampled RSS512 MiB、disk空き8 GiBで停止する。
1 GiB=1,073,741,824 bytesのtarget budgetより低い有限guardを置くが、512 MiBは20ms間隔の
観測による停止でありhard cgroupではない。container全体は16 GiB/4 CPU相当で、
1 GiB containerや専用1/2 vCPUに隔離して測定したという意味ではない。
host全体のOOMを目指さず、guardに触れたcaseは失敗として停止する。

所要時間はclientをspawnする直前から、全client完了と故障対象の回復確認まで。
この区間のprocess CPUにはclient、handler、記録vectorの確保・追記・移動、fault処理を含む。
故障後の既定10ms再起動delayが低負荷stageのwall throughputを制約する場合も含めて報告する。
1回のcallは入場待機・mailbox・handler・reply受信までのroundtrip。
p50/p95/p99は**成功したcallだけ**のnearest-rankで、rawに全timingを残す。

計時はNagiのmonotonic `clock_ns`。観測adapterはclient終了ごとに1回lockする。
JSON構築と転送は計測区間終了後だが、**process peak RSSにはJSON観測用allocationも含む**。
sample vectorはJSON転送後に解放する。checkpointは登録前、全ready、公開shutdown後、
Nagi scopeでhandleを解放+固定50ms後の4点。FD/task差0を要求する。
RSSのallocator retentionは実値で残し、RSSが元へ戻らないことだけをActor leakと判断しない。
正確なguard/context/state Drop保証は#187の別の公開API契約テストが担当する。

実測と原データは[2026-10-27 active-work結果](../results/public-actor-active-work-2026-10-27/README.md)。
Linux x86_64だけの短時間実測で、長時間安定性、Go Actor、インターネットDDoS耐性へ外挿しない。

公開版0.2.2をhard 1 GiB・1/2 CPUへ隔離した追加測定は
[0.2.2 cgroup結果](../results/public-actor-hard-cgroup-0.2.2-2026-10-27-reviewed/README.md)に保存した。
`prepare_public_actor_cgroup_image.py`は信頼するnative binaryと`ldd`で解決したlibraryを
network-free scratch contextへコピーする。生成したimageを次のcontrollerへ渡す。

```sh
python scripts/prepare_public_actor_cgroup_image.py \
  --binary /absolute/nagi-native-binary --context /absolute/new-context
docker build --network=none -t nagi-actor-bench:measured /absolute/new-context
python scripts/public_actor_cgroup_probe.py --image nagi-actor-bench:measured \
  --output /absolute/new-results --clients 4 16 64 128 --cpus 1 2 \
  --iterations 5000 --rounds 3
```

controllerはDocker HostConfigのmemory/swap/NanoCpus/cpuset/PID/network/read-only/capabilityを
各caseで検査し、cgroup v2のmemory peak・OOM event・CPU・PIDをcheckpointで保存する。
Docker socket権限が必要であり、外部や本番へ負荷を送らない。image buildと実行の前後にも
空き8 GiBを要求する。
