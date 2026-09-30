# 性能の読み方

数値は [../PERFORMANCE.md](../PERFORMANCE.md) と `benchmarks/results/`にあります。CPUは100,000要素のkernel全体をns/opで測り、ns/itemも出します。1件のprimitive演算の単独latencyを直接測った値ではありません。

同じmachine、release opt-level=3、LTO無効、warmup、7反復のmedianを使用します。データ作成はCPU kernelの時間から外し、入力とchecksumを同一にします。Rust、Nagi、CPythonを別々に実行します。

HTTPはwrkを使い、全serverを1論理CPUへ固定します。bodyとresponseを一致させ、64 keep-alive接続、2 load-generator thread、3反復を測ります。p50/p95/p99/max、socket/status error、server CPU/RSS/context switchも保存します。closed-loop試験なのでoverload時のopen-loop latencyを評価したものではありません。

allocation counterはRust GlobalAllocへの要求を呼び出しthread内で数えます。reallocはallocationにも含め、要求byte数の累計を示します。SQLite内部C allocatorや別threadのallocationは数えません。RSSとallocated bytesは別の指標です。counterは無効時もTLSの分岐を行うので測定コードの影響があります。

静的cost reportは発生箇所を示します。ループ回数、runtime内部、optimizerによる削除を含めた動的回数ではありません。hardware cache miss、branch miss、instruction count、allocator fragmentationの精密測定は未実施です。
