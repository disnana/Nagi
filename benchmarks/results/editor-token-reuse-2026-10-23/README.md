# Editor token再利用候補の棄却記録（2026-10-23）

**token再利用最適化は不採用。この候補のcompiler本体・依存testはbaselineへ戻した。**
0.2.2の実装済み高速化として扱わない。成果は再現可能な比較harnessと不利な結果を含む観測である。

## 対象と条件

- baseline: main `9fe5e079eb23ed4aef515fabd19ed86dc67958f7`、binary SHA-256 `bb2ec4aeec0f1c17e26cb249874d9cb05c0da26f4dc786a872274713b05c560a`。
- 最終候補: [棄却patch](rejected-candidate.patch)、binary SHA-256 `44eda75df8bdaeeadbbb78ce29af66ad1ec985f2a808104297906850d26f75a7`。
- toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, LLVM 23.1.1, `x86_64-unknown-linux-gnu`。
- 両binaryは同じcheckout・toolchain・既定release profile、`cargo build --release --locked -p nagic --bin nagic`、jobs=2でbuildした。baseline binaryをsource変更前に保存した。release profile/依存変更なし。
- `PATH=/workspace/toolchains/cargo/bin:$PATH`, `CARGO_HOME=/workspace/toolchains/cargo`, `RUSTUP_HOME=/workspace/toolchains/rustup`, `CARGO_NET_OFFLINE=true`。
- 各caseで同じ物理source path・overlay・requestを使用し、baseline/currentの実行順をsampleごとに反転。OS cache warm、one-shot CLI、IDE/result cacheなし。最終計測中に別のbuild/testは実行していない。CPU affinity・clock固定・machine独占はしていない。
- `20`/`500`は通常caseのhelper関数数・locals caseのlocal数。`lexical`は別の合成caseでhelper一個と20/500 KiBの単一文字列。通常program全体の代表値とみなさない。

## 設計と不採用理由

`Sources`がparser由来tokenを保持するため、位置indexが同じtextを再lexする処理を省けると考えた。
最初は`symbols::{index_impl,import_exports}`と`editor::{checker_query,Positions}`でrequest内借用を行った。
modeが拡張子と違うpublic load/native入力だけは従来lexへfallbackし、High/Low/check/ownership/Policy/
protocol/cache/generationは省略・変更しなかった。全対象回帰と比較JSONの同値確認は成功した。

しかし500localsの通常assistで悪化が再現した。`editor.rs`を戻し、symbols/import_exportsだけに
狭めた最終候補でも悪化が残ったため、最適化本体と候補依存testをすべて取り下げた。
code layoutやallocatorの影響は仮説にとどまり、根因は未特定。単に再lexが減ることを速度改善の
証明にしない。今回の判断は実行時間の観測に基づく。

## 時間: 最終の狭い候補、各15sample

[Raw timing](editor-queries.json)にwall/CPU/RSSの全sample、中央値/min/max、binary/request/outputのhashと
専有work directoryを保存した。JSON全体を毎回baselineと完全比較し、候補名・型・UTF-16位置・
partial状態・元診断・import queryには独立したassertも実施した。

| case | baseline wall中央値 ms | 候補 wall中央値 ms | 候補の増減 | baseline CPU中央値 ms | 候補 CPU中央値 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| symbols-high-20 | 10.490 | 9.872 | -5.9% | 8.610 | 8.758 |
| symbols-saved-low-20 | 9.933 | 10.245 | +3.1% | 8.301 | 7.501 |
| symbols-lexical-20 | 8.172 | 8.617 | +5.4% | 5.906 | 6.094 |
| assist-no-query-20 | 11.499 | 11.721 | +1.9% | 9.521 | 9.601 |
| assist-no-query-lexical-20 | 8.696 | 9.150 | +5.2% | 7.157 | 7.436 |
| assist-locals-20 | 11.855 | 11.583 | -2.3% | 9.129 | 9.382 |
| assist-member-20 | 11.938 | 11.970 | +0.3% | 8.881 | 10.048 |
| assist-import-20 | 2.677 | 2.688 | +0.4% | 1.235 | 1.125 |
| symbols-high-500 | 87.983 | 87.316 | -0.8% | 84.182 | 83.754 |
| symbols-saved-low-500 | 94.844 | 91.736 | -3.3% | 92.191 | 89.717 |
| symbols-lexical-500 | 12.530 | 11.864 | -5.3% | 11.112 | 10.929 |
| assist-no-query-500 | 108.786 | 111.274 | +2.3% | 106.747 | 109.203 |
| assist-no-query-lexical-500 | 16.784 | 15.239 | -9.2% | 14.452 | 12.865 |
| assist-locals-500 | 128.957 | 136.639 | +6.0% | 127.239 | 133.139 |
| assist-member-500 | 116.819 | 117.157 | +0.3% | 114.867 | 114.687 |
| assist-import-500 | 30.075 | 29.266 | -2.7% | 28.040 | 27.140 |

棄却を決めた`assist-locals-500`はwallが128.957 [123.558,139.532] →
136.639 [132.521,155.152] ms、CPUが127.239 [119.373,138.095] →
133.139 [127.910,154.230] ms（中央値[min,max]）。同じsample番号のpaired wall差は
15/15で候補が遅く、+3.57〜17.19 ms。字句負荷caseやimportの短縮があっても通常assistの悪化を
相殺して採用しない。[広い候補の前段raw](rejected-broad-slice.json)も保持し、遅いcaseを除外していない。

## Memory: 別run、各3sample

Pythonから直接forkしたchildの`wait4` RSSはlauncherのhigh-waterに影響され得るため、上のtiming
runのRSSからメモリ改善率を主張しない。別runは小さいC launcherをexecした後にcompilerをforkし、
そのchildの`wait4`を観測した。[probe source](measure_child.c)、[raw memory](editor-memory.json)。
以下はKiB、中央値[min,max]。少数sampleであり一般的なmemory上限の保証ではない。

| case | baseline peak RSS KiB | 候補 peak RSS KiB |
| --- | ---: | ---: |
| symbols-high-500 | 37548 [37484, 37664] | 36684 [36576, 36804] |
| symbols-saved-low-500 | 38980 [38652, 39120] | 37948 [37692, 38040] |
| symbols-lexical-500 | 10516 [10440, 10656] | 9964 [9740, 9964] |
| assist-no-query-500 | 43320 [43180, 43356] | 43148 [43140, 43156] |
| assist-no-query-lexical-500 | 12184 [12140, 12188] | 12020 [11912, 12152] |
| assist-locals-500 | 52712 [52648, 52764] | 52384 [52376, 52384] |
| assist-member-500 | 47672 [47640, 47724] | 47564 [47468, 47696] |
| assist-import-500 | 9532 [9416, 9660] | 8540 [8424, 8556] |

## 再現

baselineと候補を別binaryへ保存してから実行する。候補は未採用patchを隔離checkoutへ適用した検証用であり、
現在のmain/0.2.2の動作と混同しない。

```sh
python3 scripts/benchmark_editor_queries.py --baseline /path/to/baseline --compiler /path/to/candidate --baseline-revision 9fe5e079eb23ed4aef515fabd19ed86dc67958f7 --compiler-revision rejected-token-reuse --profile-label release --work-dir /tmp/editor-query-work --output /tmp/editor-query-timing.json --repeat 15
cc -O2 -Wall -Wextra -Werror -o /tmp/measure-child benchmarks/results/editor-token-reuse-2026-10-23/measure_child.c
python3 scripts/benchmark_editor_queries.py --baseline /path/to/baseline --compiler /path/to/candidate --baseline-revision 9fe5e079eb23ed4aef515fabd19ed86dc67958f7 --compiler-revision rejected-token-reuse --profile-label release --resource-launcher /tmp/measure-child --work-dir /tmp/editor-query-work --output /tmp/editor-query-memory.json --repeat 3
```

C probeはLinux用の任意memory観測で、compiler/runtimeの依存ではない。計測harness自体は`wait4`非対応環境で
CPU/RSSをnullにする。候補patch SHA-256: `7517935102c0a62ad8043c3299b854c2cdeca6567d10e3467ab951637b59ae82`。
候補段階のsource token unit・editor_assistance・module_symbols・symbolsは成功したが、このtoken再利用変更は
最終成果物に含めない。後続の[定義line検索の限定](../editor-line-search-2026-10-23/README.md)とは別の変更である。
採用しない本候補のため全workspace/native速度・4 OSの高速化検証は行っていない。

棄却patchは空白だけのcontext行を避けるzero-context形式で保存した。baselineへ適用する場合は
`git apply --unidiff-zero rejected-candidate.patch`を使う。元の通常context patchと適用後treeが同一であることを確認済み。
