# 0.2.1準備: 重複解析とCI依存ビルドの削減

未リリース。base `e2d728006dbfe49eb8a382f88ce777f29284ed4c`。
今回の変更はphysical sourceの2回目のlexを除き、parserが使ったtoken配列をloaderへ渡す。
完全High/Low検査と生成、Cargo起動、世代公開・cache整合性の判定は省略しない。
関数unitsの対象範囲と既定release設定も変えない。

## アプリの完全検査

Linux x86_64の共有host、他のローカルbuild/testなし。入力とoutは同じphysical path、
実行順を交互にし、各compilerで7回ずつ測った。compilerは両方release最適化。
smallは20関数、modulesは16 module×100関数、deep-importsは32段×50関数。
保存Lowはbaselineが生成した同じfileを両compilerで独立して検査した。
すべてのsampleで成功・診断一致、別途lowerで生成Lowのbyte一致を確認した。

| 入力 | 変更前の中央値 | 変更後の中央値 |
|---|---:|---:|
| small High | 3.01 ms | 3.06 ms |
| small 保存Low | 2.80 ms | 2.84 ms |
| modules High | 172.47 ms | 171.45 ms |
| modules 保存Low | 220.77 ms | 217.46 ms |
| deep-imports High | 175.02 ms | 169.54 ms |
| deep-imports 保存Low | 225.23 ms | 224.87 ms |

最小・最大、CPU秒、peak RSSと全sampleは[raw JSON](frontend-checks-release.json)を参照。
表は中央値だけを丸めた値で、統計的有意性は検定していない。最大約3%の短縮、
小入力は約0.04ms増であり、すべてのアプリで大きく速くなるとは主張しない。
このprobeでRSS増加は観測しなかったが、import traversal中の親token保持は長くなる。
debug compilerでも[別計測](frontend-checks.json)を残した。debugとreleaseの数字を混ぜない。

比較compilerのSHA-256:

- baseline release: `93f7cbe30a4b069c29d8541bbef02e4c3f44090df7d4b3e8ce719c4b26432ad4`
- candidate release: `95272f76f73e0fbfd3cd3623889b4e0eb102ac16dd6b00156dd92f8a24b63ebd`

baselineはexact baseの`git archive`から作成し固定した。別checkoutでcompiler自身の
Cargo targetを共有した際、candidate buildがFreshのままでbaselineと同じbinaryが
残る観測があった。source mtimeを保持した別checkoutでの現象で、原因は完全には調査していない。その結果は比較に使わず、candidateのrelease compiler artifactを
`cargo clean --release -p nagic`して再compileし、別SHAを確認した。これはNagiアプリの
CAS改ざん再現ではない。以後の比較でもCargo Fresh表示だけをcompiler provenanceにしない。

再現:

```sh
python scripts/benchmark_frontend_checks.py \
  --baseline /absolute/path/to/baseline-nagic \
  --compiler /absolute/path/to/candidate-nagic \
  --work-dir /absolute/path/to/owned-work \
  --output /absolute/path/to/result.json --repeat 7
```

## Repositoryのテスト・CI

同じdebug compiler `7aca1206c5b15eb112b50896b594ee802aa8d3d772841b519e6d6fc9797f4eca`
でfrontend fixtureのcache配置だけを変更した。各1回、23件すべて成功。
変更前のCargo起動時にP0 compilerが再buildされたため、fixture-beforeもP0込みである。

| 条件 | test harness | Cargo込み |
|---|---:|---:|
| 変更前: 4つの個別empty target | 221.11 s | 232.092 s |
| 変更後: 1つの共有empty target | 76.71 s | 78.025 s |
| 変更後: 既存のwarm共有target | 4.38 s | 6.001 s |

emptyはCargo targetだけで、registry/OS cacheはwarm。試行1回なので中央値や一般的な
CI短縮率にしない。coldとwarmを直接同条件の比較にもしない。source/outは独立、
共有するのはCargo targetだけ。明示cacheの値を3回の実Cargo起動で照合し、fixtureの
既存directory/sentinelを保持し新所有rootだけDrop削除することも確認した。
[条件・raw logの一覧](ci-cache-improvement-summary.json)を残す。

regression jobで後続testが使わないrelease compiler buildを1step除いた。
testはdebug `CARGO_BIN_EXE_nagic`を使い、package jobのrelease compilerと配布検査は残る。
変更前main CI `39722534177`では当該stepがLinux66s/macOS ARM92s/Windows90sだった。
これは履歴のstep時間であり、新candidateのCI全体の短縮・料金はまだ測っていない。
初回CI policyは149件、新harnessのpure test11件を加えた最終確認は160件成功。
既存のstep・assert・deadline・security gateを削っていない。

## 8ケースの通常devとunits比較

同じcandidate release CLI、実runtimeの専有copy、100個のprimitive関数と集約関数、
別module・async main/readyを使った。各方式3回、trialごとに順序交替、source/runtimeの
physical pathとseed lockは同じ。構成ごとにempty targetを使い、各構成内では編集を累積した。
runtime copyはworkspace.membersだけをruntimeへ絞り、元runtimeを変更していない。
これは今回変更前後の比較ではなく、既存のordinary devとopt-in unitsの比較である。

| ケース | ordinary中央値（最小–最大） | units中央値（最小–最大） |
|---|---:|---:|
| clean | 37.079 s（36.919–38.386） | 37.104 s（36.901–37.599） |
| 変更なし | 0.530 s（0.528–0.565） | 0.445 s（0.441–0.461） |
| 関数本体 | 0.536 s（0.531–0.537） | 0.593 s（0.586–0.687） |
| 公開API | 0.513 s（0.503–0.518） | 0.607 s（0.595–0.613） |
| 複数module | 0.515 s（0.494–0.517） | 0.624 s（0.593–0.631） |
| async本体 | 0.539 s（0.524–0.545） | 0.589 s（0.576–0.601） |
| runtime依存編集 | 1.524 s（1.482–1.598） | 1.448 s（1.420–1.457） |
| 不正lock拒否 | 0.032 s（0.031–0.032） | 0.036 s（0.034–0.040） |
| lock復元後の回復 | 0.529 s（0.527–0.549） | 0.474 s（0.445–0.485） |

変更なしは約16%短縮したが、本体・API・複数module・async編集ではunitsが約9–21%遅い。
この小さいfixtureではcrate追加費用が勝ち、cold依存compileも改善しない。
unitsを既定へ変更する根拠はない。履歴から自動的に最適な分割を選ぶ機能も未実装。

本体編集でunitのCargo Freshは6/8、APIは7/8、asyncは8/8だった。
runtime編集はsource hit9/9でもunit Freshは0/8、compiled artifactは11となり、
Cargoが依存のnative再compileを行った（ordinaryは2 artifact）。source hitとnative hitは異なる。
各構成8個の成功exe、計48個の出力/終了と旧bytesを繰り返し検証し、6回の不正lockは
Cargo.lockのparse拒否・native公開なし・旧latest/exe保持を確認して、元lockを復元した。
runtime変更は専有copyの無害な定数変更であり、外部依存version更新の性能比較ではない。
CAS source改ざん・alias・孤児Cargo・公開失敗は既存generation73件の回帰で確認した。

peak RSS中央値はclean約767/771MiB、warm約485–487MiB。論理file sizeによるtarget容量は
構成終了時ordinary約2,469MiB、units約2,474MiB、outは約628/630MiBだった。
hardlink重複を含む論理sizeは実disk占有と異なる。専有targetは各構成完了後に削除し、
inputs/out/旧exe/raw logを保持した。これはnative実行性能やCI全体の測定ではない。
[summary](smart-matrix-summary.json)と[全sample](smart-matrix.raw.json.gz)にCPU/RSS/容量、
Fresh/Dirty、source hit/miss、失敗理由と各exe hashを残す。

```sh
python scripts/benchmark_smart_build_matrix.py --compiler /absolute/path/to/nagic \
  --runtime-root /absolute/path/to/Nagi --work-dir /absolute/path/to/owned-work \
  --output /absolute/path/to/new-result.json --repeat 3 --functions 100 \
  --prune-owned-targets
```

## Native実行と保証の限界

実runtime、既定release、Rust adapterの`black_box`で反復数を渡す5,000万回loopを使った。
両方build後に実行順を交互にして各7回、全checksum一致・旧exe hash不変。
中央値は192.61ms→193.21ms、範囲は190.81–209.38ms→190.71–197.37ms。
生成RustのSHA-256は両方
`94efbdbc1609ba4c8af99c0054fc01759399471c21dcc8c83d6be80cec0296b6`で一致。
この差から実行性能の改善・劣化を結論しない。HTTP throughputの測定でもない。
[全sample](release-native.json)と[probe](release_cpu_probe.py)を残す。
baseline build時は依存92 artifactの再compileが発生し、currentはwarmだったため、
このprobeのbuild時間は速度比較に使わない。

新しいquery/result cacheは追加していないので、今回独自の無効化条件はない。
すべての入力を再parse/checkし、既存CASは生成Rust・manifestを照合し、nativeの
toolchain/profile/target/features/依存/build script等の判定は毎回Cargoへ委ねる。
async/所有型/未知callなどのunits fallback条件とopt-inは既存どおり。

parser/metadata/診断/overlay/Unicode・CRLF・改行なしの回帰、実native frontend23件、
fmt/clippy、全workspace検査は成功。全workspaceはnetwork許可下で670.09秒、既存ignoreを維持。
最初のsandboxによるsocket拒否のlogも保存した。これは同条件での速度比較ではない。
CI policy160、CSRF deadline inventory37、利用者CI generator7も成功。
exact-head CIと独立最終auditはPRの実行結果と照合し、この記録だけでリリース可能と宣言しない。
