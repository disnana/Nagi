# Nagi 0.2.2 公開Actor: hard 1 GiB・1/2 CPU有限実測

公開版`nagi-v0.2.2`のLinux配布物と同梱runtimeを使い、公開`std.actor` APIで
独立stateを更新する実Nagiプログラムを測定した。計測用Nagi sourceで対応client数を
128まで広げたが、compiler/runtimeの製品コードは変更していない。

各clientは別Actorへ5,000回の`actor.call`を直列に送り、外側と内側が`Ok`で
replyがそのActorの`i + 1`と一致した回だけを成功とする。拒否、timeout、業務Err、
queue投入、故障対象の`REPLY_LOST`は成功に含めず、retryもしない。

Docker cgroup v2でtarget containerを**1,073,741,824 bytes memory、swap 0、
CPU quota 1/2、同数のcpuset**へ固定した。networkなし、read-only rootfs、全capability削除、
`no-new-privileges`、PID上限512である。client・時計・timing vectorはtarget process内にあり
CPU時間とmemory peakに含む。外側Python controllerとDocker daemonはtarget cgroup外である。

## 結果

24 process、合計**6,360,000成功返信、正常失敗0**。4/16/64/128 active client、
各5,000件、hard CPU 1/2、各3 roundである。全caseで別の故障対象Actorへworker Errを1回送り、
公開eventからFAILED 1、RESTART_SCHEDULED 1、STARTED generation 2、factory初期state 0を確認した。

| hard CPU | active clients / Actor計 | 成功/s 中央値 [min, max] | CPU ns/成功 中央値 | p50 µs | p95 µs | p99 µs | cgroup peak 最大 MiB |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 4 / 5 | 1,527,553 [1,181,574, 1,626,077] | 645 | 3.425 | 4.477 | 8.213 | 7.59 |
| 1 | 16 / 17 | 1,498,467 [1,473,499, 1,513,266] | 665 | 15.493 | 19.589 | 73.752 | 25.64 |
| 1 | 64 / 65 | 1,363,860 [1,289,766, 1,364,784] | 733 | 65.148 | 136.877 | 303.083 | 97.73 |
| 1 | 128 / 129 | 1,481,229 [1,321,085, 1,572,703] | 675 | 106.599 | 239.786 | 390.559 | 193.67 |
| 2 | 4 / 5 | 1,788,431 [1,728,358, 1,800,396] | 857 | 0.761 | 2.524 | 4.256 | 7.73 |
| 2 | 16 / 17 | 2,374,436 [2,143,829, 2,422,732] | 829 | 1.983 | 19.239 | 29.484 | 25.87 |
| 2 | 64 / 65 | 2,198,168 [2,123,554, 2,431,104] | 855 | 0.951 | 60.602 | 134.994 | 97.78 |
| 2 | 128 / 129 | 2,251,778 [2,210,847, 2,425,177] | 870 | 6.349 | 123.687 | 254.335 | 194.88 |

latencyは成功callの直前からreplyの型・値検査後までで、入場待機、mailbox、handler、
reply受信を含む。各caseでnearest-rankを計算し、表は3 roundのquantileの中央値である。
throughputの分母は全client完了と故障回復確認までのphase wallで、既定10ms restart delayも含む。

CPU ns/成功はtarget processのCPU clockを正常成功数で割った値であり、handlerだけの費用ではない。
2 CPUでは全client段階で処理量が増え、p99も改善した。一方、成功1件当たりCPU時間は
全段階で1 CPUより増えた。1 CPUでは64 clientが16 clientより遅く、2 CPUでは64/128 clientが
16 clientより遅いため、ここから無制限な並列追加を推奨しない。
共通活動区間のclient別完了数Jain index中央値は1 CPUで全段階ほぼ1.0、2 CPUでは
4/16/64/128 clientで0.724 / 0.559 / 0.041 / 0.140だった。2 CPU・64 clientでは
3 round中2 roundで共通活動区間を定義できず、中央値は定義できた1 roundだけの値である。
これは各clientの開始から最初に終了したclientまでの短い共通区間だけの値であり、
scheduler全期間の公平性とは断定しない。

最大cgroup memory peakは**204,349,440 bytes（194.88 MiB）**、最大memory currentは
89,550,848 bytesだった。全checkpointでkernelの`memory.max=1073741824`、
`memory.swap.max=0`、`cpu.max=100000/100000`または`200000/100000`、
cpuset=`0`または`0-1`、`pids.max=512`を照合した。`memory.events`のmax/OOM/OOM-killは0、
最終Tokio task差も0である。memory currentの残留はallocator保持を含み、RSSだけで全allocationの
即時返却を証明しない。

## 故障中の継続と回復

故障区間は別Actorへの失敗call直前からgeneration 2・state 0確認までである。
全24 caseで区間中にも正常Actorが成功返信を完了した。故障区間中の正常完了中央値は、
1 CPUの4/16/64/128 clientで16,208 / 16,440 / 14,556 / 12,654件、
2 CPUで20,000 / 22,309 / 18,328 / 20,567件だった。

これはworkerが`Err`を返すone-for-one再起動の検証である。panic、OOM、abort、native crash、
非協調的CPU処理の隔離保証、process間memory isolation、BEAM同等性は示さない。

## 同一性と再集計

- release tag: `nagi-v0.2.2` / `7c92045a04519609ae287803a31ac0816c728086`
- release archive SHA-256: `0dc1b8d42d5283c7ba41663a1420f041ed2461e6ced8b3eadfcc0383be1ee0b0`
- compiler: `nagic 0.2.2`, SHA-256 `2c917055b4ec1aba5d6695f750101bb4a8956a8535c58cb42eb261446c83fa19`
- native binary SHA-256: `f4198164f93f0f11e27c420ef58183fac4ee15c015f5425dfb54c64581431e60`
- Docker image: `sha256:e77fe059ccc555603adc87bac908be6acd362d8fa5aefdd41f4852796e2a187e`
- Rust: 1.99.1 (`58cf8f9c35b355199727627d07ae34a4e2846d67`), Docker server 28.4.0
- host: AMD EPYC 9V74 virtual host, Linux x86_64

`summary.json`はcaseごとのhard設定、cgroup checkpoint、全集計を持つ。`*.jsonl.gz`は
Nagi processが出した全call timingと回復tuple、`aggregate.json`は3 roundの分布、
`verification.json`は24 rawの独立再集計、`provenance.json`はsource/build/image hashを持つ。

```sh
python benchmarks/public-actor-active-work/verify_cgroup_results.py \
  benchmarks/results/public-actor-hard-cgroup-0.2.2-2026-10-27-reviewed
```

## 限界

短時間のi64 counter-messageで、HTTP、I/O、mailbox飽和、長時間soak、Go Actor、
Python/Elixir比較、ネットワークDDoSを測っていない。1 GiBでの最大Actor収容数も求めていない。
DockerのCPU quotaとcpusetでtargetを制限した結果で、物理core予約や無負荷専用hostではない。
共有hostのばらつきがあるため、3 roundの範囲を併記し、単一の最大値をSLAにしない。
