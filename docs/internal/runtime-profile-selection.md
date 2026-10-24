# 検査済みIRに基づくruntime依存の選択

基点は`d58b895`。0.2.2向けの未リリースopt-inであり、公開0.2.1・既定fullの置換ではない。
既存の生成本体/units CAS、完全High/Low検査、immutable世代、Cargoによるnative判定を再実装しない。

## 狙いと境界

保存済みのPoint初回観測では、全体62.895秒のうちCargoが62.884秒、198 artifactがCompiledだった。
単発のdev/debug0/jobs2・offline・seeded lockであり、比較中央値ではない。
frontend検査を省くより、使わないHTTP/auth/TLS/DNS依存を小さいプログラムから外す候補を先に検証する。
この段階から一般バックエンドの速度やCI全体の費用削減を推測しない。

## 選択の契約

CLIは`build/run --runtime-profile full|auto`、既定full。force-coreは設けない。
autoはsealed CheckedProgramのemission programと各function/expression/statement planを確認する。
全生成関数・型を対象とし、未使用定義を落とすreachability最適化はしない。
同期primitive/単純record・local/既知直接call・print・対応演算/fieldの閉じた集合だけを許可し、
未知の型・構文・標準operation、async、extern、属性、native path/reexport等はfullへ戻す。
名前や生成Rust文字列からruntime依存を推測しない。所有権やPolicyを別のsolverで再判定しない。

追加native入力・Rust adapter・Rust依存はfull。incremental要求もfullを保ち、既存CAS schema2/3の
path-only runtime依存形とキー計算を変更しない。coreは通常のmonolithic生成/schema1で使う。
runtime未対応のfallbackと、Cargo/lock/cache失敗の扱いは異なる。後者をfull再試行で隠さない。

## Cargoとruntime

元の`nagi-runtime`は全依存・全公開APIを保持し、直接Rustからの`default-features=false`でも変えない。
新しい`nagi-runtime-core` packageは`[lib] path="../runtime/src/lib.rs"`で同じruntime sourceを共有する。
core packageの定数build scriptだけが`nagi_runtime_core` cfgを出し、full部分を除外する。
Cargo featureの組合せやdefault無効化で元runtimeのAPIが消える構成にはしない。
このcfgを出すbuild scriptもCargoのfingerprint対象であり、Nagiの独自native cacheへ置き換えない。
coreにはError、JSON、Serde、Tokio、actor/concurrent/task/metrics/result、およびrusqliteと
database::FromRow/indicesを残す。Point生成がFromRowを要求するため、SQLite依存を削除しない。
HTTP/auth/html/publicSQLite/securityとHTTP応答helperは元runtimeの全構成で維持する。
SQL sessionのauth observerや認証lifecycle自体は変更しない。

対応する元runtimeとcore manifestにschema付きcapabilityと固定sibling/package/libpathの宣言・検査を設ける。
古い/未対応runtimeはfullへ戻す。coreのstaging/projection両manifestだけに同じpackage/pathと
`default-features=false, features=["core"]`を指定し、Cargoにもfeatureの存在を検証させる。
この宣言は外部runtimeソースの安全性の証明ではなく、任意Rustの監査境界を拡張しない。

coreとfullは異なるCargo packageで、生成アプリ内では既存の`nagi_runtime`名へaliasする。
Rustのnominal type identityを二つのpackage間で混在させず、adapter・追加Rust依存・native入力ではfullを使う。
配布物にはcore manifest/build scriptと共有runtime sourceの両方を含め、workspace継承・NOTICEの依存閉包も検査する。

Cargo.lockを削らず継承し、Cargoは毎回実行する。feature/profile/toolchain/target/build-script/
環境/依存変更のnative有効性はCargoへ委ねる。Nagi独自の意味hashでnativeを復元しない。
root選択能力の観測後にruntimeが変わればCargo失敗を含め通常の失敗処理へ進む。
出力lock・新bin名・snapshot・旧exe保持・atomic latest・run前unlockは維持する。

build/runは最初の投影書込み前にinstallation rootを解決し、既知のruntime manifest・共有libを
既存の入力identity保護へ加える。coreの固定宣言またはpackage/lib/buildのidentityがある場合は
core manifest/build scriptも含め、pre-lock/post-lockの双方でsymlink/hardlinkを含む別名を拒否する。
単にruntime-coreという名前の旧アプリ出力をruntime入力とは見做さない。任意のRust include依存を
完全に探索する機能ではなく、check/lowerのRust不要という契約も維持する。

## 観測と検証

statsは`runtime:{requested,selected,reason}`を追加する。理由は公開[説明](../runtime-profiles.md)の表にまとめる。
通常fullのmanifest形と既存release設定は変えない。完全検査・RustEmission検証・Cargoを省略しない。

plannerのHigh/Low positive/negative、未使用async/未知operation/入力境界、runtime能力、CLI引数を検査する。
実nativeではHello/Point、同out/共有targetでfull→core→full、継承lock・壊れたlockでの旧成功保持、
adapter/SQL/HTTP等のfallback、旧receipt/CASの互換性を確認する。default fullの既存全suiteと4 OS CIは別gateである。

`benchmark_runtime_profiles.py`は既存smart-build harnessのmeasurement/receipt/cleanupを再利用し、
Hello/Point×full/autoを3回以上、交互順・同compiler/source/runtime/seed lock・個別empty targetで測る。
body/API/runtime編集とlock破損・回復、全旧exeのbytes/実行も確認する。完了した専有targetだけを回収する。
時間・CPU・最大子RSS・Fresh/Compiled・容量・native出力を記録し、速くないcaseも残す。
実行性能はprocess startupと出力の確認に限り、本番throughputの比較とは扱わない。
