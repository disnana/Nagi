# 世代別ビルドの測定

Phase 1のcompilerとPhase 2のcompilerで、小さなアプリのビルド待ち時間を比較した。これはHTTPの処理性能やアプリ実行速度の測定ではない。

## 条件

- 共有Linux環境、Rust/Cargo 1.98.1、Python 3.12.14。
- 標準出力へ整数を出すアプリと、依存のない空のローカルruntimeを使用。実際のCargoとrustcでreleaseビルドし、各実行結果を確認。
- compilerはdebugビルド。比較元のcommitとSHA-256は`baseline-compiler.json`、比較先のSHA-256は各`after` JSONに記録。
- 各compilerにつき、空の専用Cargo targetから1回、ソース変更なしで7回、整数を変更して7回。OSのページキャッシュは初期化していない。
- 最初と再測定はcompilerごとに続けて実行。最後は旧・新を1回ずつ交互に実行し、先に実行する側も交互に変更。順序は`paired-order.json`。
- 合計90ビルド・90実行。各回のコマンド、所要時間、ソース・生成Rust・実行ファイルのハッシュ、サイズ、標準出力をJSONに保存。Cargo診断は対応する`*-diagnostics/`に保存。

## 観測

中央値、単位ms。各再ビルド欄は7回の測定。

| 比較 | 変更なし・旧 | 変更なし・新 | ソース変更・旧 | ソース変更・新 |
|---|---:|---:|---:|---:|
| 初回 | 77.53 | 91.92 | 73.91 | 92.24 |
| 再測定 | 78.37 | 110.12 | 78.72 | 82.84 |
| 交互実行 | 81.92 | 80.58 | 89.77 | 87.76 |

最初の比較には増加があり、交互実行ではほぼ同程度だった。共有環境の負荷と実行順の影響を除けていないため、高速化・性能不変・一定の追加コストのいずれも、この結果から保証できない。空のtargetでのビルドは各比較1回しかなく、傾向の判断に使わない。最小・最大と全生データは`summary.json`と各測定JSONにある。

45組すべてで入力ソース、生成Rust、実行結果が一致した。実行ファイルは旧458,032 byte、新458,152 byteだった。世代別のbin名も異なるため、120 byteの差を特定の機能のコストとは断定しない。

throughput、request latency、allocation数、RSS、生成Futureのサイズは今回測っていない。従来のFutureが比較Rustより32 byte大きかった件は、この測定では解決を確認していない。大規模ソースのsnapshot書き込みや長期的な世代保持のディスク使用量も別途測定が必要。

## 再実行

`benchmarks/build_generations.py --help`で指定方法を確認できる。例:

```sh
python benchmarks/build_generations.py \
  --compiler /path/to/phase1/nagic \
  --label before \
  --compare-compiler /path/to/phase2/nagic \
  --compare-label after \
  --directory /tmp/nagi-generation-comparison
```

再実行にはRust/Cargoが必要。`--directory`には未使用のディレクトリを指定する。結果はそのディレクトリ内の`measurements.json`、交互比較では`primary/measurements.json`と`comparison/measurements.json`に保存する。ビルド中に旧世代やキャッシュを削除する方式は使っていない。初回の測定時のharnessは`measured-initial-harness.py`、交互測定時のharnessは現在の`benchmarks/build_generations.py`と同じSHA-256で、記録は`measured-source-sha256.json`と測定JSONにある。
