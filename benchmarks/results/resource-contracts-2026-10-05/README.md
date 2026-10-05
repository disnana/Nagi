# 資源定義の集約前後: check/lowerの測定

2026-10-05。4つの既存fixtureについて、check/lowerを各warmup 1回＋測定7回、順次実行した。前後それぞれ64起動が成功し、全反復と前後の未正規化Low bytesが一致した。全文Low/Rust goldenは別のlibrary testでも維持している。

| fixture | check 前→後 ms | lower 前→後 ms |
|---|---:|---:|
| http-inspection | 206.414 → 209.104 | 214.474 → 207.243 |
| http-borrow-mappers | 200.457 → 204.761 | 204.704 → 204.390 |
| actor-data | 140.221 → 133.695 | 142.271 → 143.782 |
| data-derives | 12.211 → 10.914 | 11.049 → 10.910 |

中央値には増減がある。共有hostのwarm反復で、process起動・frontend・CLIのfile書込み・pipe収集を含む。Python側のhash・記録保存はtimer外。性能不変・高速化・因果的なoverhead上限を保証する測定ではない。Cargo、runtime throughput、allocation、RSS、CPU、Future sizeは測っていない。

集約前の参照は先行test-only head `ca9362e5`。後のfixture修正はproductionを変えていない。集約後はhead `eb93873`に未commit差分を適用したCLIで、git statusを環境記録に残した。CLIの埋込みbuild commitや個別rustc flagsは推測しない。前後で入力のphysical path、cwd、output path、sample数を揃え、測定中に別のCargo処理は実行していない。

## 記録

- `before/`と`after/`: 環境、CLI/input/script hash、全64起動のcommand/cwd/時間/exit/stdout/stderr、summary、出力hashの再照合。
- `comparison.json`: 全8caseの中央値・min/maxと前後bytes一致。
- `measure.py`: 実際に使ったPython stdlibだけの採取script。一般向けのbenchmark APIではなく、同条件の測定記録である。

未正規化の出力は作業環境で保存し、そのbytes/hashを記録した。tracked fixtureの全文goldenは固定logical ModuleIdを使うため、ここで比較したphysical ModuleIdの出力とは分ける。

## 同じ条件で採取する

Rust/Cargoの環境と保存CLIを用意し、記録済みと同じfixture path/cwd・work pathで実行する。result directoryは新規にする。集約前のscriptは保存CLIのSHA-256を照合するため、別環境で再buildしたexeが同じhashになるとは限らない。

```sh
python measure.py \
  --compiler /tmp/nagi-container-flow-target/debug/nagic \
  --repository /workspace/Nagi-boundary-foundation \
  --directory /workspace/test-tools/compiler-rust-boundary-plan/phase3-frontend-comparison/after-repeat \
  --work-directory /workspace/test-tools/compiler-rust-boundary-plan/phase3-frontend-comparison/work \
  --label after --samples 7 --warmups 1
```

採取時はbeforeを完了してから集約後を測定し、同じwork directoryへのwriterを重ねない。条件を変えた結果を、この前後測定と同じ比較として扱わない。
