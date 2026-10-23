# 0.2.2候補: symbols定義位置の探索を物理lineへ限定

`symbols::index_impl`はclass/enum/functionの各定義を位置へ戻すたびに、物理fileの全tokenを
先頭から走査していた。既存のclass field検索と同じ`partition_point`で対象lineの先頭へ移り、
そのline内だけを既存のkeyword/name条件で検索する。新しいcache、token借用、parse mode fieldは加えない。
[棄却したtoken再利用](../editor-token-reuse-2026-10-23/README.md)とは別の変更である。

## 同値条件と境界

lexerは`src.lines().enumerate()`の順でtokenを追加する。通常token/Newlineと途中のIndent/Dedentは
現在の物理line、終端Dedent/EOFは`src.lines().count()+1`なので、token.lineは単調非減少である。
位置indexは従来と同じlex結果を使い、ASTやline offsetを変更しない。

以前の検索は`pair[0].line == local`で全配列をfilterして最初のkeyword/name一致を選んでいた。
新検索は同じlineの連続区間へ開始位置を絞るため、候補の順序・条件・first-matchを維持する。
`pair[1]`のlineへ追加条件を付けず、keywordがline末尾にあるpairも従来と同じ扱いにする。
Lowの同一line複数定義、canonical/resolved/display名、native合成lineから物理lineへのoffset、
Indent/Dedent/EOF、CRLFとUTF-16の既存変換を維持する。定義数D・token数Tに対し位置pair検索は
O(D×T)からO(D×log T＋対象lineのtoken数合計)へ減る。他のmetadata/checker検索費用は残る。

High/Lowのparse/check、型・ownership・Policy、editor partial/acceptance、元診断・protocol、
Cargo・generation/cache/旧exeは変更しない。通常check/build/runの経路は変更していない。

## 測定条件

- baseline source: main `9fe5e079eb23ed4aef515fabd19ed86dc67958f7`。
- 候補source: docsのみのcommit `99c3fc5a4fadad17d30fc76e56d29dd0c0730de8`を土台とする上記symbols変更。docs commitのcompiler/Cargo sourceはbaselineと同一。
- baseline binary SHA-256: `bb2ec4aeec0f1c17e26cb249874d9cb05c0da26f4dc786a872274713b05c560a`。
- 候補 binary SHA-256: `17f53488bfc7b347776e3f340f175f1d85288377f4b548328782fc8597ad5168`。
- 両者とも`rustc 1.99.1 (58cf8f9c3 2026-10-13)` / LLVM 23.1.1 / x86_64 Linux、同じ既定release profile、`cargo build --release --locked -p nagic --bin nagic`、jobs=2。baselineを変更前に別名保存した。release設定・依存は変更していない。
- 同じ物理source path・overlay・request、sampleごとの交互実行順、OS cache warm、one-shot CLI、checker/IDE cacheなし。timing中に別のbuild/testは実行していない。CPU affinity/clock固定・machine独占はしていない。
- `20`/`500`は通常caseのhelper関数数・locals caseのlocal数。`lexical`だけは別の合成caseでhelper一個と20/500 KiB文字列。公開fixtureをscript内で生成し、private入力は使っていない。
- reportのrevision/profileラベルは実行時に明示した。harnessの既定はunknownで、binaryからrevisionを推測しない。

## 時間: 各15sample

[Raw timing](editor-queries.json)にはwall/CPU/RSS全sample、中央値/min/max、binary/request/output hashを保存した。
JSON全体を毎回baselineと完全比較し、候補・型・UTF-16位置、元診断、partial状態、import contextには
独立assertも行った。全caseの同値確認が成功した。

| case | baseline wall ms 中央値[min,max] | 候補 wall ms 中央値[min,max] | CPU中央値 ms baseline→候補 |
| --- | ---: | ---: | ---: |
| symbols-high-20 | 9.693 [8.847,10.638] | 9.744 [9.201,10.932] | 7.478→7.684 |
| symbols-saved-low-20 | 10.004 [9.031,11.128] | 9.927 [8.681,10.618] | 8.032→8.121 |
| symbols-lexical-20 | 7.827 [7.338,8.419] | 7.759 [7.236,8.469] | 4.908→6.121 |
| assist-no-query-20 | 11.283 [10.815,13.661] | 11.284 [11.000,12.199] | 9.393→8.778 |
| assist-no-query-lexical-20 | 8.363 [7.728,9.511] | 8.519 [7.944,9.526] | 6.539→6.276 |
| assist-locals-20 | 11.169 [10.755,11.890] | 10.888 [10.652,12.097] | 9.044→8.328 |
| assist-member-20 | 11.286 [10.828,13.719] | 11.359 [10.624,11.884] | 9.391→9.266 |
| assist-import-20 | 2.447 [2.317,2.594] | 2.556 [2.395,2.947] | 1.165→1.124 |
| symbols-high-500 | 85.482 [80.344,90.810] | 84.734 [80.290,91.710] | 83.357→82.730 |
| symbols-saved-low-500 | 91.264 [88.347,99.483] | 90.265 [88.396,108.868] | 89.158→88.313 |
| symbols-lexical-500 | 12.376 [11.592,13.281] | 12.188 [11.519,14.125] | 9.491→9.557 |
| assist-no-query-500 | 109.402 [101.650,115.640] | 107.407 [102.205,125.194] | 106.722→104.722 |
| assist-no-query-lexical-500 | 15.840 [15.406,29.186] | 15.989 [15.444,19.870] | 13.987→14.289 |
| assist-locals-500 | 124.968 [121.129,135.844] | 123.623 [119.828,149.245] | 123.436→122.003 |
| assist-member-500 | 119.517 [116.674,146.322] | 116.597 [113.629,128.385] | 117.396→114.680 |
| assist-import-500 | 30.058 [29.250,34.913] | 30.382 [29.164,31.916] | 27.402→28.706 |

通常500関数/locals caseの中央値差はおおむね1〜2%程度で、ばらつき・計測誤差を含む。
小caseの改善は一貫せず、直接変更していないimport caseも遅いsampleを含む。
棄却token案で再現した通常assistの6〜10%悪化は今回のrunでは見られない。
この結果を普遍的な高速化率、Rust/Cargo build時間、アプリ実行速度やCI全体の改善として扱わない。

## Memory: 別run、各3sample

[Raw memory](editor-memory.json)は小さいLinux C launcherからcompilerをforkしたchildのwait4。
Python launcher由来のhigh-waterを避けるため、timing runとは別にした。
[probe source](../editor-token-reuse-2026-10-23/measure_child.c)を共有する。KiB、中央値[min,max]。
少数sampleであり、メモリ改善・上限の保証は主張しない。

| case | baseline peak RSS KiB | 候補 peak RSS KiB |
| --- | ---: | ---: |
| symbols-high-500 | 37412 [37352,37544] | 37220 [37212,37424] |
| symbols-saved-low-500 | 38788 [38764,38972] | 38588 [38572,38868] |
| symbols-lexical-500 | 10628 [10432,10704] | 10236 [10232,10252] |
| assist-no-query-500 | 43296 [43164,43304] | 43264 [43140,43300] |
| assist-no-query-lexical-500 | 12168 [12132,12252] | 12144 [11924,12144] |
| assist-locals-500 | 52708 [52684,52840] | 52376 [52256,52736] |
| assist-member-500 | 47604 [47544,47736] | 47344 [47216,47564] |
| assist-import-500 | 9508 [9504,9524] | 9288 [9172,9348] |

## 検証と再現

`cargo test --locked -p nagic --test symbols --test module_symbols --test editor_assistance`成功。
新しい境界回帰はHighのIndent/Dedent/CRLF/EOFと、Lowの同一line class/enum/function・EOFを検査し、
定義名/kind/元file/line/UTF-16 columnを独立期待へ照合する。既存の保存Low、import、native/@replace、
overlay、recovery、type/moveエラーとprotocol境界も成功した。
`cargo clippy --locked --all-targets -- -D warnings`、`cargo fmt --all -- --check`、Python構文検査も成功。
全workspaceの`cargo test --locked`もLinux x86_64で成功（Cargo offline、実socket許可、jobs=2、test threads=4、dev/test debug情報=0、debug assertionsは既定のまま）。独立最終auditと4 OS CIは別gateであり、ローカル成功から推定しない。
測定後、差分をmain `9fe5e079`からの独立branchへ移した。compiler・依存・測定harnessのbytesは上記hashのままで、Docs PRを前提としない。

```sh
python3 scripts/benchmark_editor_queries.py --baseline /path/to/baseline --compiler /path/to/candidate --baseline-revision 9fe5e079eb23ed4aef515fabd19ed86dc67958f7 --compiler-revision symbol-definition-line-search --profile-label release --work-dir /tmp/editor-query-work --output /tmp/editor-line-timing.json --repeat 15
cc -O2 -Wall -Wextra -Werror -o /tmp/measure-child benchmarks/results/editor-token-reuse-2026-10-23/measure_child.c
python3 scripts/benchmark_editor_queries.py --baseline /path/to/baseline --compiler /path/to/candidate --baseline-revision 9fe5e079eb23ed4aef515fabd19ed86dc67958f7 --compiler-revision symbol-definition-line-search --profile-label release --resource-launcher /tmp/measure-child --work-dir /tmp/editor-query-work --output /tmp/editor-line-memory.json --repeat 3
```

環境変数とsource fingerprintは[metadata](metadata.json)を参照。measurement専用のtarget/cache/保存binaryは
local-onlyにあり、配布物やcompiler依存へ加えない。
