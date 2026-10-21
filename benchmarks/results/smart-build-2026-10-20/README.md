# アプリ開発ビルド基盤の第1差分・測定記録

対象base: `2c703b89645b164f586a926e473d08395be65c90`。2026-10-20〜21 UTCにLinux x86_64で測定した。公開0.2.0後の開発ソースへの未コミット変更であり、配布済み0.2.0の機能・性能を更新した意味ではない。

小さなアプリのcold native buildをdev profileで短縮できた。一方、warm buildの大幅な短縮、意味差分による成果物再利用、CI全体の短縮は今回実証していない。変更観測と整合性receiptを先に実装し、完全検査・Cargo・新しい世代への公開を毎回維持した。

## 比較条件

[測定script](../../../scripts/benchmark_project_builds.py)と[全sample](raw.json)、[終了状態](benchmark-status.json)、[生成物照合](artifact-audit.json)を参照。最終runは346.32秒、終了コード0、`complete: true`。baselineはbaseのrelease compilerを退避したもの。current compilerも最適化してビルドした。

- Rust/Cargo 1.99.1、Linux 6.18.55、x86_64、見えるCPUは5。共有host上で順番に実行し、この最終比較中は他のNagi検査を並走させていない。CPU固定や独占hostではない。
- 実runtimeを使う同じ2ファイル・同じcanonical source pathのアプリ。構成ごとに新しいCargo target/outを作成。registryとOS cacheは温まっており、依存download時間を含まない。`CARGO_NET_OFFLINE=true`。
- 通常アプリには同じrepository Cargo.lockをseedした。解決後のlockfile全体のhashは出力先に由来するアプリpackage IDが構成ごとに異なるため一致しないが、外部214 packageのname/version/source/checksumは3構成で一致した。
- currentの通常sampleは`--build-stats`有効。cleanと編集sampleは各1回、warm/check/native実行は各3回の中央値。順序効果を除去した統計検定ではない。
- `one-file`: mainにprintを追加、`function-body`: import先の41を42へ変更、`public-api`: 戻り値i64をi32へ変更。変更を累積する同一手順。最終出力`43`と`0`を3構成×3回で確認。
- 各構成で通常buildは10世代、別CPU probeは1世代。通常out容量は10世代分。native target容量は通常とCPU probeの両方を含む。

## アプリ開発の実測

| 項目 | 既存release | current release＋観測 | current dev＋観測 |
|---|---:|---:|---:|
| 新規targetのclean build（秒、1回） | 73.295 | 70.477 | 37.440 |
| 依存cache済みwarm build（秒、3回中央値） | 0.1492 | 0.1511 | 0.1678 |
| 1ファイル編集build（秒、1回） | 0.1475 | 0.1464 | 0.1645 |
| import先の関数本体編集（秒、1回） | 0.1510 | 0.1845 | 0.1606 |
| 宣言signature編集（秒、1回） | 0.1489 | 0.1676 | 0.1645 |
| Rust不要のcheck（ms、3回中央値） | 2.576 | 2.508 | 2.611 |
| cleanの最大child RSS（MiB） | 709.01 | 710.24 | 772.70 |
| Cargo target容量（MiB、両workload） | 828.39 | 828.39 | 1962.52 |
| 通常out容量（MiB、10世代） | 5.00 | 5.05 | 44.23 |
| 通常アプリnative exe（bytes） | 445,048 | 445,048 | 4,553,464 |

このsampleのdev cleanは既存release比48.9%短縮、current release比46.9%短縮。ただしwarmは速くならず、devのメモリ・ディスク使用は増加した。release同士のclean差を変更の効果と断定しない。通常アプリの最終生成Rust SHA-256は3構成で同一。公開APIの既定profileはreleaseのままで、devを本番最適化の代わりにしない。

current releaseのwarmではCargo artifactが200 fresh / 1 compiled（99.50%）、devでは197 / 1（99.49%）。これは既存Cargo依存cacheのartifact割合であり、Nagi関数cacheやCI cacheのヒット率ではない。変更なしでも世代固有binは再コンパイルされる。関数本体とsignatureの変更はそれぞれ`implementation_changed` / `interface_changed`として観測したが、影響グラフで検査やビルドを省略していない。

最後の編集状態で観測を外したwarm 3回の中央値は既存release 0.1500秒、current release 0.1478秒、dev 0.1640秒。通常warmとはソースの状態も実行順も違うため、この差を観測処理単体の費用としない。RSSは`wait4`によるchildの最大high-water markであり、全プロセスの同時使用量やheap profileではない。

## native実行性能の確認

別の小さいCPU probeで、native `black_box`から得た500万回のループと剰余加算checksumを3回実行した。正しいchecksumを全構成で確認した。

| 項目（3回中央値） | 既存release | current release | current dev |
|---|---:|---:|---:|
| CPU probe（ms、process開始込み） | 20.316 | 20.509 | 18.208 |
| 通常アプリのprocess開始〜終了（ms） | 1.145 | 0.943 | 0.881 |

releaseの約1%差はこのsample数・共有hostでは回帰とも同等性能の証明とも判断できない。devの数値を一般的な性能優位と解釈しない。HTTP/DB throughput、負荷下のtail latency、allocation量の測定ではない。Rust/Axum・FastAPI・Goとの条件をそろえた比較は未実施。

CPU probeには通常アプリのseed lockをコピーしていない。offlineで別に解決した外部package集合はCPU probeの3構成間では一致するが、通常workloadから16 packageのversionが変わっている。このためtarget容量には2つの依存graphが含まれ、単一アプリの最小cache容量ではない。照合結果は[artifact-audit.json](artifact-audit.json)に保存した。

## 検証結果と限界

[checks.json](checks.json)と[tests.json](tests.json)にcommand、時間、log hashを保存した。

- 全workspaceの`cargo test --locked`は119個のtop-level suiteすべてが成功summaryで終わり、最後のdoctestまで失敗なし。log作成〜最終更新は1215.49秒。途中の実行controller再接続でprocess終了コードの取得は失われたため、`exit_code: 0`とは記録していない。既存ignoredは維持した。開始時点は最後の観測用OS-path記録追加より前。
- 最終production変更後のbuild generation harnessは54 passed / 0 failed、27.51秒。project harnessは13 passed / 0 failed、100.56秒。profile、整数overflowの既存契約、旧exe不変性、native/import変更、失敗時の旧latest維持、壊れた観測記録、非UTF-8 source/cwd等を含む。
- fmt、Clippy全target warnings-as-errors、optimized compiler build、no-default-features check成功。Pythonのgenerator 7件、receipt 5件、既存CI policy 107件が成功。最終cache path変更後にgeneratorを再実行し、Ubuntu版・3 OS版の生成YAMLもparseした。
- 最終測定のcurrent release/devそれぞれ8個、合計16個のreceiptをPython検証器で再照合し、compilerファイルのhashを含め一致した。receiptは破損検出であり、攻撃者に対する署名やnative再利用許可ではない。
- 日英Docsのwebsiteは102ページを生成。実装前設計確認と実装後の別agentによる独立auditを行い、最終cache修正も再確認した。source reviewをnative/CI実行の代わりには数えない。

全workspace testのbaselineとの同条件時間比較、アプリ独自testの時間、生成workflowのremote実行、macOS/Windows/ARM、IDE GUI、ホットリロード、CI costの数値は未確認。Nagiにはアプリ共通の`test` commandがないため、check/build時間をアプリtest時間と偽っていない。

## CIの基準値

baseのGitHub Actions [run 39444177134](https://github.com/disnana/Nagi/actions/runs/39444177134)の観測全体時間は2005秒（33分25秒、開始〜最後のjob終了、待機含む）、linux jobは1985秒。[取得記録](ci-baseline.json)を参照。これはNagi repositoryの既存CIであり、利用者アプリのCI時間ではない。今回の変更後のremote CI、生成workflowでのcacheヒット率、請求額は未測定で、短縮率を主張しない。

生成workflowはnative不要のcheckを先に実行し、OS/arch/toolchain/Nagi/manifest/lockでcacheを分離する。生成lockfileも引き継ぎ、default branchだけsave、PRはrestore。GitHub cacheはforkから可読であり、秘密やprivate dependencyをcacheしてよい設定ではない。[CI guide](../../../docs/user-ci.md)の読取境界とself-hosted条件を参照。

## 再現と次の差分

```sh
# baselineはbase revisionから別途release buildしたcompilerを指定
CARGO_NET_OFFLINE=true python3 scripts/benchmark_project_builds.py \
  --compiler target/release/nagic --baseline /path/to/base/nagic \
  --work-dir local-only/bench --output local-only/comparison.json --repeat 3
```

`--runtime-root`の既定はこのrepository。Rust、Cargo、cache済み依存が必要。scriptは一意の作業directoryを保持し、既存cacheや世代を削除しない。[bootstrap-baseline.json](bootstrap-baseline.json)は初回compiler/helloの別測定、[exploratory.json](exploratory.json)は並走検査・異なるlock条件を含む初期調査であり、上表に混ぜていない。

次はruntime依存featureと共有targetによるcold build削減、安定した本物のCargo libraryと世代固有thin binの分離を小さく比較する。frontend query cache、移植可能なcontent-addressed成果物、影響範囲によるtest選択は完全な依存閉包と独立した整合性検証ができるまで有効にしない。[調査・優先順位・transport案](../../../docs/internal/smart-build-plan.md)を参照。
