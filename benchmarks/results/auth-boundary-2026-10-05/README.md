# Auth boundary比較

Linux x86_64 / Rust 1.98.1。Axumの同一実行ファイルで`NAGI_AUTH_POLICY_MODE`だけを切り替え、手書きRustのload/policyとNagi生成Rustのload/policyを比較した。Router、SQLiteデータ、auth fixture、serialization、status/payload、4096 byte body上限、1000 ms期限、32request容量、panic応答、listener/停止経路は共通。TLS/HTTP2/独立header・送信・停止期限/connection admissionは無し。

最終`rust` modeのhandlerはNagi関数を呼ばず、手書きRust load/policyを通る。Nagi modeは`load_document`/`read_policy`を生成したRustで実行する。両方でRust verifier/保護DB操作と生成DTO/permission markerを共有する。言語全体、Axum対標準Hyper、JWS/本番認証の速度比較ではない。結果をHTTP backend置換の根拠にしない。

8接続・HTTP/1 keep-alive・closed loop・1秒warmup＋3秒を各3run、順序を交互に実行した。最終測定は`NAGI_THREADS=2`。負荷clientもstd Rustの実socket。全6runのtransport/status/body errorは0。CPU/RSSはLinux `/proc`。同hostでclient/serverが動くため、独立負荷機のSLOや最大throughputを保証しない。RSSはload前後のsnapshotでpeakではない。

| 最終3runの観測 | Nagi生成Rust | 手書きRust |
|---|---|---|
| qps範囲 | 49.8–69.1k | 49.8–69.4k |
| qps中央値 | 62.7k | 64.1k |
| p99中央値 | 325µs | 354µs |
| server CPU秒中央値（3秒run） | 5.39 | 5.32 |
| RSS snapshot範囲 | 4.20–4.33 MB | 4.33–4.40 MB |

分布が重なり、3runの共有host観測から速度の優劣を断定しない。正確なraw値は`results.json`。初回のpolicyだけの比較（実worker4、両modeともNagi loadへ入る）は`initial-results.json`/`initial-micro.jsonl`に保持し、上の最終比較と混ぜない。

Futureの観測例はPrincipal8 byte、Grant16 byte、Nagi policy72 byte/Rust policy64 byte、Nagi load168 byte/Rust load144 byte。policyとGrant mint/consumeのmicroは呼出threadで追加allocation0。`yield_now`だけのpolicy futureをnoop wakerで直接pollし、入力をblack_boxへ渡す小fixtureで、Tokio scheduler/HTTP/DBの費用は含まない。ns値はoptimizerの影響も受けるためsocket測定に置き換えない。型layoutの普遍的ABI保証ではない。

共有binaryは4,008,032 byte。既存dependency cacheから同じ生成projectを3回rebuildし、wall 5.04/4.94/5.00秒。各logで生成crateを再compileしている。clean buildやRustとNagiの別binary build時間を測ったとは扱わない。Highと保存Lowの各52実HTTPcaseは両modeで成功し、panic/handler timeout/partial-body timeout後の応答とlistener解放も確認した。

実行:

```sh
rustc --edition=2021 -O benchmarks/auth_boundary_load.rs -o /tmp/auth-boundary-load
NAGI_THREADS=2 python benchmarks/auth_boundary.py \
  --executable build/application-example-verification/auth-boundary/high/nagi-main-aaa9421f094081c9 \
  --client /tmp/auth-boundary-load --output benchmarks/results/auth-boundary-2026-10-05
```

実行ファイル名は生成Cargo.tomlで確認する。`results.json`にbinary/client hash、環境、個別run、CPU/RSS/error counts、`micro.jsonl`にFuture/allocator観測を保存する。build時間は既存dependency cacheを使う同じprojectの再buildとして別`build-times.json`へ保存する。clean buildの費用を測ったとは扱わない。
