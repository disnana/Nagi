# Frontend sealing比較（2026-10-05）

Phase 1前後でHigh parse/check → Low文字列生成 → 独立Low parse/check → Rust文字列生成を比較した。新経路はLowの最終checkを`check::finalize`で行い、私有facts/planを封印してからemitする。width 1/16/64の入力・Low・Rust計9ファイル、および既存正例17 fixture × 3出力の51ファイルは、名前集合と内容を直接比較してすべてバイト一致した。[output-equality.json](output-equality.json)に各サイズ・両側SHA-256を保存した。有限のfixtureの一致であり、全言語意味論の同値性を証明するものではない。

基準はmain `ded4c44cd3ebf984b382995322cefc769b4a6cb3`（承認済み計画`b156e05`と同じtree `7ed5f49db8a4b59188347fc5e5d4dd1fcaf38de4`）。旧libraryから作った測定実行ファイルを保持し、library更新後に旧版を再測定した。新側はPhase 1の作業tree。実行ファイル・測定用sourceのhash、toolchainと環境を[environment.json](environment.json)、読戻し時点のcompiler source hashを[compiler-source-sha256.txt](compiler-source-sha256.txt)に記録した。後者はbuildとsourceの対応を証明するattestationではない。

Rust 1.98.1 / Linux x86_64 / Debian 13の共有container。standalone harnessはrustc既定のopt-level=0、libraryはCargo debug・最適化なし・debug情報0・incremental無効。CPUはAMD EPYC 9V74、見える論理CPUは5、cgroup CPU quotaは4 CPU相当。専用CPU・affinity・周波数固定はない。測定順は旧版初回→新版→旧版再測定で、交互・無作為化実験ではない。

各widthはview containerの再束縛とnested Result matchを含む関数組を生成する。10回warmup後、100 pipeline反復の経過時間を100で割るsampleを5本取った。下表は旧版**再測定**との比較で、単位は1 pipeline当たりms。

| width | High / Low / Rust bytes | 旧版中央値（sample範囲） | Phase 1中央値（sample範囲） |
|---|---:|---:|---:|
| 1 | 496 / 713 / 2,536 | 3.180（3.022–3.352） | 2.338（2.118–2.493） |
| 16 | 7,528 / 9,620 / 31,303 | 42.848（39.074–45.134） | 25.305（20.330–28.825） |
| 64 | 30,088 / 38,180 / 123,953 | 174.979（152.130–185.851） | 79.691（75.116–86.215） |

今回のdebug workloadでは新版の時間が短かった。一般的なfrontend高速化、release性能、native adapterや実ファイルのprovenance処理費用へ外挿しない。旧版初回のtimingも別ファイルに残し、再測定と混ぜない。

| 全widthを処理するprocess全体 | 旧版再測定 | Phase 1 |
|---|---:|---:|
| 最大RSS（KiB） | 10,144 | 12,656 |
| user CPU秒 | 106.087 | 53.236 |
| system CPU秒 | 0.086 | 0.055 |

RSSはprocess全体の最大常駐量で、warmup・setup・各widthを含む。allocation数、1反復の追加allocation、個々のchecked factsの生存量を測った値ではない。Future size、Rust backendのcompile/build時間、生成アプリのruntime速度も未測定である。

測定harnessは[compiler_frontend_sealing.rs](../../compiler_frontend_sealing.rs)。旧libraryに対して`--cfg baseline`、新libraryに対して通常cfgでcompileする。それぞれ対応するcompiler revisionから用意したlibraryを`--extern`へ指定する。

```sh
# 旧版libraryでcompileする場合だけ --cfg baseline を加える。
rustc --edition=2021 benchmarks/compiler_frontend_sealing.rs \
  --extern nagic=/path/to/corresponding/debug/libnagic.rlib \
  -L dependency=/path/to/corresponding/debug/deps \
  -o /tmp/frontend-sealing
/tmp/frontend-sealing /tmp/frontend-sealing-output
```

この例は生成物とtimingの再取得用で、process資源のcollectorは含まない。実測時に使用した[旧harness](measured-before-harness.rs)と[新harness](measured-after-harness.rs)も保存した。両者の異なるAPIを同じ処理段階へ接続しており、Cargoや生成アプリの実行はtimed regionへ含めない。生成物本体はtask workspaceの`/workspace/test-tools/compiler-rust-boundary-plan/baseline-repeat-output`と`after-output`に保持し、PRには含めない。両側の内容hashは[output-equality.json](output-equality.json)に記録した。

raw timingは[旧版再測定](frontend-before-repeat.jsonl)、[新版](frontend-after.jsonl)、[旧版初回](frontend-before-initial.jsonl)。process資源は[旧版](frontend-before-resources.json)、[新版](frontend-after-resources.json)、集計は[summary.json](summary.json)。corpusの入力とhashは[corpus-inputs.json](corpus-inputs.json)。corpus各fixtureの出力はLow、直接AST由来Rust、保存Lowを再parse/checkしたRustの3種類で、CLI・module/native統合・全プログラムの実行等までこのバイト比較だけで検証したとは扱わない。
