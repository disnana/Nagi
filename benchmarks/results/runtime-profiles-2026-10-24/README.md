# runtime profile比較（2026-10-24）

0.2.2向け開発ソースの`--runtime-profile auto`を、同じコンパイラの既定fullと比較した記録です。公開済み0.2.1の機能ではありません。[利用方法](../../../docs/runtime-profiles.md)と[設計](../../../docs/internal/runtime-profile-selection.md)を参照してください。

## 条件

計測した実装は[修正commit](https://github.com/disnana/Nagi/commit/236913f7a6f75b6a7d14cc0584c9c049072f8bf8)と同一treeで、コンパイラSHAは`49aa30fbf2b0eec6067755f1c1b72408e9a0d650d6862f2652b3fd4ecf887f9c`です。独立レビューで確認した共有libの別名保護も含みます。

Linux x86_64、Rust/Cargo 1.99.1、各構成3回。Hello/Pointそれぞれでfull/autoの実行順を交互にし、同じ入力・runtimeパス・seed Cargo.lockを使いました。dev、debug情報0、2 jobs、offlineです。各構成のCargo targetは空から開始し、registryとOS cacheは温まっています。ネットワーク取得や通常devのdebug情報を含む時間ではありません。

seed lockは計測runnerが出力先へ配置します。通常buildの既存出力lock継承と、lockがない場合のCargoによる解決は変更していません。index更新・downloadを含む初回の総時間とは区別してください。

測定対象コンパイラのSHA、実装tree、ソースfingerprint、harness SHA、全条件と中央値・範囲は[summary.json](summary.json)、各サンプルは[samples.jsonl](samples.jsonl)にあります。絶対ローカルパスと内部ログは公開データから除外しています。以下はこの環境と対応する小さいプログラムの結果です。HTTP・SQL・asyncを含む一般バックエンド全体、Windows/macOS、CI全体の短縮率へ一般化できません。

## ビルド時間

単位は秒、中央値です。括弧内は3回の最小–最大です。

| ケース | Hello full | Hello auto | Point full | Point auto |
|---|---:|---:|---:|---:|
| empty target | 59.238 (58.932–59.328) | 11.314 (11.186–11.642) | 60.682 (59.678–61.423) | 11.410 (11.182–11.573) |
| 変更なし | 0.157 | 0.100 | 0.304 | 0.142 |
| 関数本体変更 | 0.163 | 0.102 | 0.300 | 0.139 |
| 公開形状変更 | 0.163 | 0.096 | 0.295 | 0.139 |
| runtimeソース変更 | 1.070 | 0.297 | 1.207 | 0.371 |
| lock破損の拒否 | 0.014 | 0.014 | 0.015 | 0.015 |
| lock復元後 | 0.194 | 0.093 | 0.302 | 0.139 |

coreはHTTP/auth/TLS/DNS等の依存を除いた別Cargo packageを使います。fullの既存Rust APIは維持します。High/Lowの完全検査とCargoは毎回実行しており、Nagi独自hashによるnative再利用ではありません。未対応IRや追加Rustコード、incremental要求はfullへ戻ります。

## CPU・メモリ・容量

| empty target時の中央値 | Hello full / auto | Point full / auto |
|---|---:|---:|
| 子プロセスCPU時間（秒） | 86.12 / 21.29 | 88.77 / 21.67 |
| 最大子プロセスRSS（MiB） | 616.29 / 350.51 | 616.60 / 350.73 |
| Cargo target容量（MiB） | 575.51 / 150.04 | 577.65 / 150.79 |
| Compiled artifacts | 198 / 50 | 198 / 50 |

RSSは同時実行した全プロセスの合計ピークではありません。artifact数は異なるcrate数ではありません。通常のwarm buildは両方ともCompiled 1、Freshはfull 197、auto 49でした。依存数が異なるため、Fresh率の比較をcache hit改善とは扱いません。runtime変更では両方ともCompiled 2でした。

## 正しさと変化しなかった点

84回のビルドのうち72回が成功、12回は意図したlock破損の拒否です。破損時の旧latest・旧実行ファイル保持、復元後の成功、receipt検証を確認しました。対応するfull/autoの生成Rustは36組で同一でした。完了後に専有Cargo targetを整理し、公開済み72実行ファイルのSHAと実行結果を再確認しています。

メモリと起動時間もサンプルに残しています。Helloの変更なし時の最大子RSSは98.28 / 98.11 MiB、native起動時間は1.294 / 1.437 msでした。Pointの起動時間は1.396 / 1.426 msです。短い起動時間の差から本番throughputの改善は主張しません。

既定releaseについては、旧コンパイラと新コンパイラが**同じfull runtime**を使うCPUループを別に確認しました。既定release設定と生成Rustは同一。各15回、2回warmup後の交互実行で、wall中央値は旧20.074 ms / 新20.038 ms、範囲は19.931–20.798 / 19.890–23.773 ms、RSS中央値は旧22,732 / 新22,732 KiBでした。この入力の測定だけから実行性能の改善や全般的な同等性は判定しません。これはcoreのthroughput比較でもありません。[再現用probe](release_cpu_probe.py)とsummaryの`default_release_cpu`に入力・設定・サンプルを残しています。

## 再現

```sh
cargo fetch --locked
python scripts/benchmark_runtime_profiles.py --compiler path/to/nagic --work-dir build/runtime-bench --output build/runtime-bench.json --repeat 3 --prune-owned-targets
python benchmarks/results/runtime-profiles-2026-10-24/release_cpu_probe.py --baseline path/to/old-nagic --compiler path/to/nagic --runtime-root . --work-dir build/release-probe --output build/release-probe.json --repeat 15
```

Python 3.11以上を使います。測定runnerは既存smart-buildの計測・receipt・専有target整理を再利用します。成功していないtargetや利用者のcacheを削除しません。ソースruntime内への作業先・レポート先指定も拒否します。実際に使用したharnessとコンパイラのSHAを記録しています。

複数モジュール・async/runtime機能ごとの速度、native成果物の共有、query cache、履歴からのcrate自動分割、CI全体の時間・費用、他言語との比較はこの測定の対象外です。既存full fallbackの回帰テストと各OSのCIは、上のLinux測定とは別に判定します。既存のCI方針では通常PRのL2がLinux/Windows/macOS arm64、正式releaseのL3がmacOS Intelを含む4 OSです。
