# 初回の依存lockを選ぶ（0.2.2）

[English](en/lock-seeds.md)

公開済みNagi 0.2.2の明示opt-in機能です。0.2.1以前にはありません。既定は従来どおりCargoによる依存解決です。対応する配布物には、配布runtimeと照合した初期lockを`runtime-seed/`に同梱します。

```sh
nagic build --project my-app --seed-lock --build-stats
nagic run --project my-app --seed-lock --runtime-profile auto --profile dev --build-stats
```

`--seed-lock`はbuild/run限定の明示指定です。出力先に`Cargo.lock`がなければ、対応配布物のlockを最初のCargo入力として使います。既存の出力lockは常に優先し、コメントも従来の条件で維持します。既存lockが壊れていても配布lockで置換・再試行しません。Cargoは毎回起動し、実際の生成manifestに合わせてlockを再判定します。通常のHigh/保存Lowの完全検査、既定release、世代・旧実行ファイル保護は変わりません。

これはvendoringではありません。registryのindex・crate downloadやRust/Cビルドは依然必要な場合があり、オフライン利用やビルド時間短縮を保証しません。配布lockの依存集合は、生成アプリが最終的に使う集合と一致するとは限りません。依存更新を自動追随する仕組みでもありません。

[3回ずつのLinux測定](../benchmarks/results/lock-seed-2026-10-25/README.md)では、空のtargetからのfullビルドは通常約46〜47秒・seed約58秒で、seedの方が遅くなりました。同じCargo targetを使う別アプリでは短縮しましたが、依存版の違いもあり、Nagi単独の効果や一般的な高速化率とは扱えません。測定はdev・debug情報0・2 jobs・offlineでregistry/OS cacheが温まった条件です。再現性を重視する場合にこのopt-inを選び、通常解決の既定は維持します。

## 選択結果

`--build-stats`の`lockfile`を確認できます。

```json
{"lockfile":{"requested":true,"source":"bundled","reason":"bundled_seed"}}
```

| source / reason | 意味 |
|---|---|
| `output` / `existing_output_lock` | 出力先の既存lockを使用 |
| `bundled` / `bundled_seed` | 検証した配布seedを初期入力として使用 |
| `cargo` / `not_requested` | 指定なし。従来の解決 |
| `cargo` / `runtime_unsupported` | seedがない旧配布・ソースcheckout、未対応schema/runtime版 |
| `cargo` / `incremental_requested` | 生成本体/関数単位CASでは従来の解決 |
| `cargo` / `native_input`, `rust_adapter`, `rust_dependencies` | 追加入力・Rust adapter・追加Rust依存があるため従来の解決 |

対応schemaのseedを選んだ後、内容・固定配置・hash・runtime identityの不一致を検出した場合は失敗します。Cargo失敗をseedなしで再試行しません。seedのないソースcheckoutを配布物と同じ条件だと考えないでください。

指定なし・既存lock優先時は、未使用seedの内容やhashを検査しません。ただし配布seedを生成先へ上書きするaliasは拒否します。既存ファイルの同一性を確認できない場合も、安全性を推測して書き込みません。配布物の`runtime/`、`runtime-core/`、`runtime-seed/`の配置を保ってください。

[依存runtimeの選択](runtime-profiles.md)とは別の指定です。autoでcoreが選ばれても、seed使用の可否は上記の条件で独立に決まります。
