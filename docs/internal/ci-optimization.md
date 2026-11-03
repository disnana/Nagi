# CI改善の判断と測定（2026-10-20）

0.2.0の凍結候補とは別の、次期開発用変更。[段階別の支持範囲](ci-levels.md)と[nextest比較](nextest-evaluation.md)を参照。検査のtimeout、assert、seed、feature、公開成果物のコンパイルprofileは変えていない。

## 実測したボトルネック

基準は[run 39415520849](https://github.com/disnana/Nagi/actions/runs/39415520849)、HEAD `467f8c9c791b4e98d1d33e15f8b9fc5515dafe0a`。GitHubの完了job/stepのstarted_atとcompleted_atを使った単一runの観測であり、改善後との同条件比較ではない。

| 観測対象 | 経過時間 |
|---|---:|
| 最初のjob開始から最後の検査job終了 | 3,463秒 |
| 成功した21 jobの実行区間合計 | 364.53 runner-minutes |
| Linux workspace `cargo test --locked` | 1,492秒 |
| CLI回帰 Linux / Windows / ARM Mac / Intel Mac | 727 / 1,415 / 1,041 / 1,538秒 |
| Intel Macのpackageとregression job合計 | 96.30 runner-minutes |
| Windows回帰のcache復元 | 約3.53 GB、約160秒 |
| Intel packageのcache保存 | 203秒 |

runner-minutesはOS料金倍率・課金丸めを適用していない実行区間の合計。請求額、AI処理時間、runner queue待ち時間ではない。job開始前には依存待ちも含まれるため、それをqueue時間と断定しない。Linuxでは同じcache keyの予約競合も観測した。数値を一般的なSLAやnextestの速度効果に転用しない。

## 採用した構成

- 変更判定→release-plan→ci-planの後、独立したRust包括検査、OS別package/回帰、browser、IDEを別runnerで並行実行する。重いnativeテストを同じrunner内で一斉起動せず、既存のport・一時directory・Cargo lockの分離と検査を維持する。
- L1は限定したfront-end変更だけ。未分類やruntime/security/control-plane変更はL2。main/週次は3 OS、明示RCと正式配布はIntelを含む4 OS。Intelを外すことによる通常CIの削減は選択変更であり、各テスト自体が高速化したという意味ではない。
- Readyは全選択jobを確認する。公開直前に同じ計画とsource SHAを再確認する。PR・手動RC・tagから公開しない。既存のversion/CHANGELOGによる公開判定を維持し、mainの公開runを後続commitで取消しない。新しいmerge承認機構を設定済みと仮定しない。
- 失敗時にも判定が必要な6 jobは `always() && !cancelled()` とする。[GitHubの取消仕様](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-cancellation)ではjob条件がtrueなら取消後も継続するため、公開jobも取消時には拒否する。stepの後片付け用 `always()` は維持する。既に外部で受理された公開の取り消しや、旧runへの遡及は保証しない。
- 外部Actionを確認したcommit SHAへ固定。checkoutの資格情報を残さず、診断artifactは7日、配布候補は14日の明示保持期間（既存の個別指定は維持）。ソース・秘密情報・私有鍵をcacheへ追加しない。
- 歴史的なBrowserLogin追加観測とeditor-stage観測は手動専用。必須のruntime/実browser検査は本体CIに残す。editor-stageは旧source束に限定した観測で、現在のcompiler変更を受け入れる汎用性能検査ではない。元のsource bindingは維持し、不一致なら拒否する。

## cacheとRust runnerの比較

| 手段 | 今回の判断と理由 |
|---|---|
| 既存actions/cache | restore/saveを分離。既存の重いkey/復元対象は維持し、全通常step成功後のmainだけ保存。PRによる大量保存とpost-step予約競合を減らす。復元自体の費用は残るため、次の実測で判断する |
| L1専用Rust cache | `nagi-critical-v1`のRust build/dependency領域を成功PRに保存。L1はmainで実行しないため、この例外が必要。GitHubのPR ref scopeと別keyを使い、公開producerはこの領域を参照しない |
| Swatinem/rust-cache | workspace crateを既定で除外する方針などが現行native-target/cfg別buildと異なる。置換による再ビルド・保持量を未測定のため今回は採用しない |
| sccache | incremental Rustやlinkの制約があり、通信/復元費用を含む利得が未測定。導入済み・有効と扱わず、公開profileも変えない |
| cargo-nextest | checksum検証済みの版で独立評価。二つの既存compiler targetでinventoryとignored状態を照合し、build・列挙・交互順のwarm実行を別計測。libtest出力依存の診断やsocket/native全suiteはまだ移行しない。doctestはCargoで維持 |

既存dev/testのincremental無効と、releaseのopt-level=3、lto=false、codegen-units=1、panic=unwindを維持する。debug情報やlinkerの一括変更は証拠なく行わない。`check`は型検査、`build`は配布物、`test`は実行検証として目的を分ける。異なるRUSTFLAGSの診断buildを同一成果物とみなして省略しない。

## Python

7 workflowのPython処理を調査。pip依存導入は本体Linuxとwebsiteの2 jobだけで、requirements別のpip cacheが既にある。他は主にstdlib unittest/診断/配布検証。利用可能な前段run `39358061068`（HEAD `1229d412d535c3bb4b87ae27b3959fc01cec2eff`）ではsetup-pythonが0〜1秒、pip導入3秒。build_examplesの277秒は主にnativeコンパイルで、Python起動の遅さではない。

この根拠ではuvやpytest-xdistを追加しない。Pythonのsocket/native検査の並行安全性も未証明であり、不要な依存を増やさない。tests/requirements.txtは版の範囲指定、websiteは直接依存の固定で、いずれも全transitive依存のhash lockではない。cacheを依存の固定と同一視しない。

## 確認方法と未確認

repository rootで `python -B -m unittest discover -s scripts/ci -p 'test_*.py'` と `python -B -m unittest discover -s scripts/ci -p 'test_nextest_evaluation.py'` を実行する。必須CIは従来のsource/mutation guardsも実行する。許可した配線の逆変換だけで元の検査commandへ戻ること、不正なpin/matrix/公開条件・command削除を拒否することを維持する。歴史的hashの新しい値への置換ではない。

実Actionsの[L1 run 39511768690](https://github.com/disnana/Nagi/actions/runs/39511768690)（HEAD `44b9d13`、一時PR #137は未マージで終了）では、lexerのcommentだけを変更し、重要Rust検査・browser・4 IDE検証・Readyが成功した。wallは584秒、非加重job実行区間合計は31.38 runner-minutes。包括検査はこの選択では実行せず、上の基準runとは作業集合が異なるため、同条件の高速化率ではない。

4 OSの[nextest実測](nextest-evaluation.md#2026-10-20の実測)では、この軽い2対象は従来Cargoより約1.6〜2.0倍長くかかり、必須runnerは置換しない。最終HEADのL2結果・wall/job・cache観測はPRの検証欄で同一HEADを明記する。新しいL3配線の手動4 OS実行と、実プラットフォームでの取消は未確認であり、ローカルの厳格なmatrix/拒否条件検証とは区別する。L3の公開前必須ゲートは維持し、未完了から公開しない。総課金削減額や一般的な性能保証は示さない。

参照（2026-10-20確認）: [GitHub cache境界](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching)、[cache restore/save](https://github.com/actions/cache/tree/0057852bfaa89a56745cba8c7296529d2fc39830)、[rust-cache](https://github.com/Swatinem/rust-cache)、[sccache](https://github.com/mozilla/sccache)、[uvのGitHub案内](https://docs.astral.sh/uv/guides/integration/github/)、[nextest文書](https://nexte.st/docs/)。

## Linux critical pathの分割（batch 29）

PR #218のLinuxは26分55秒で、CSRF semantic mutantsが6分29秒、samplesが4分51秒、Session diagnosticsが3分16秒、release compileが2分18秒、HTTP registryが2分04秒だった。#217はLinuxが22分43秒、workspace runnerが約23分。workspace commandの22分51秒の内訳はcompile約2分03秒、実行約20分48秒で、重いbinaryは`build_generations` 179.11秒、`security_sf01_native` 101.55秒、`http_stdlib` 95.35秒、`error_routes` 94.32秒、`go_backend` 88.54秒、`auth_boundaries` 83.43秒だった。cache missを含む各runの観測値であり、変更後の予測値ではない。全体ではWindows packageが長く、CI全体の半減を主張しない。

`linux-workspace-shards`は4 runnerを使い、lockedな`cargo metadata --no-deps`でdefault workspace memberの全targetを列挙する。観測した重いbinaryと明示したfallback推定値を使い、重い順に決定的に分配する。完全なsource差分を確認できた場合だけ、担当shard内で変更したintegration targetまたはsource packageを先に実行する。未知・欠落した対応は全inventoryを優先する。変更パスによるtarget除外はなく、既存L1/L2/L3 selectorとreleaseの4 platform契約を維持する。

各shardは担当selectorだけをlocked Cargoと`--workspace`でcompileし、workspace feature unificationを維持する。全libraryは1個の`--lib` selectorにまとめ、他のselectorもmetadata上の完全なtarget集合を保持する。既存の同名`security_sf01`/`task_handles`は両packageの実行ファイルを残す。build/test mode混在、identity重複、default memberと`--workspace`対象の不一致は拒否する。kind/name/source pathで全実行ファイルを照合し、count summaryが残るpretty形式で全caseとignoredをlistする。Cargoの実行blockごとに全名前・状態・summaryを照合し、libtest固有の` - should panic`注記は正規化する。実行自体はCargoを使い、packageのcwdとbinary環境を維持する。既存CLIの0件harnessだけを明示許可し、未知の0件targetと実行0件のshardは失敗する。既定ではbuildだけ行うexampleは`cargo build --locked --workspace --profile test --example NAME`で1回compileし、集約はexact commandと成功exitも要求する。shard 0では既存の`cargo test --locked --doc`を1回実行し、exact command、成功exit、passed/ignored/failed合計、summary、metadataのdoctest対象library集合を記録する。summary数はlibrary集合と一致し、既存の非0件doctest実行を要求する。個々のdoctest名は観測の限界として残し、名前単位で照合したとは報告しない。

`linux-workspace`集約はmetadataを独立に再取得し、4件ちょうどのreceipt、各targetの担当集合、ignored整合、非0件実行、HEAD・rustc全identity・既定flags・lock・workflow hash一致を確認する。失敗、skip、欠落、重複、未知、部分実行はReadyを通らない。Readyと公開前の独立gateは`linux-csrf-mutants`と`linux-samples`も要求する。mutantのcommandとartifact stepは同じまま一緒に移し、自己完結したrelease build・tutorial・sample・socket・fuzzのblockも内部順序を維持して移す。Session diagnosticsは他stepの失敗条件と強制検査を保つためLinuxに残す。

新Linux cache keyにはrole/shard、OS、architecture、toolchain、既定compiler/doc flags、Cargo lock、workflowとrunner sourceを含める。診断flagsを含む既存namespaceへのfallbackは設けない。保存は成功したmain pushだけで、hit時も全担当実行を要求する。version計画、main/tag公開条件、platform契約は維持する。新しい外側の厳密なinverseで変更前workflow hashへ戻し、既存command-body証明と歴史hashは変更しない。

今回のCI専用branchはmain `0b21a1d`から分離し、PR #218の機能変更には依存しない。外側のinverseはこのmainのworkflowとbyte単位で一致し、新しいbase hashだけをそのsourceへ固定した。既存の歴史hashは変更していない。mainのmanifestと追跡sourceをPythonで独立照合すると、非doc targetは133件、129 selectorとdoc 1件になり、既存Goの4 platform件数は34/33/34/34のまま。これはCargo metadataやnative実行ではなく、source inventoryの検証である。selector/receipt拒否、担当targetだけを実行するmock、cache/gate mutation、Python CI policy suite全体を検査する。

先行batch 29の別sourceでは実metadata 134 target/130 selector、offline `enum_parser`の8件、feature統合と小workspaceのnative照合を観測したが、今回のmain上の実行証拠として転用しない。全workspace/native実行、artifact転送、cold/warmのcritical pathは未確認。比較時は同じsource・inventoryでcold/warmを分け、各jobの依存待ち・queue・setup・cache restore/save・compile・実行・artifact転送と、Linux gate・全体終了・runner-minutesを記録する。基準#217の全体終了を支配したWindows packageは34分52秒だった。shard weight合計は配分用の推定でActions実測時間ではない。4 runnerでの依存compileとsetup/転送の重複が短縮分を相殺する可能性があり、実測短縮やReadyの報告にはexact-source CI成功と独立reviewが必要。


### native child stdoutと外側libtestの区別

`graph_render`は同じtest binaryを`current_exe --exact image_child --nocapture`で起動し、native childのstdoutを継承する。外側にも同じ名前のcaseがあるため、stdout全体の`test ... ok`を数える方法では正常な子実行を外側の重複と誤判定した。Cargoのselector、workspace features、cwd、Nagi用environmentは維持し、実行時だけCargoのhost runnerで外側binaryごとの独立したstable libtest `--logfile`を指定する。runnerは実binaryへexecする前に自分が加えたrunner設定をenvironmentから外し、nested Cargoやself subprocessへ引き継がない。排他的な開始fileとbinary別boundaryで本当の外側の再実行を拒否し、独立event fileの全case・ignored・状態と外側の最終count summaryを照合する。native childの出力とsummaryは診断として残し、外側caseのauthorityには使わない。filterや直列化は追加しない。

`--logfile`はstableで利用できるがdeprecatedである。将来のtoolchainが拒否した場合はCargoの非0 exitで失敗し、stdoutだけの弱い検証へfallbackしない。orchestration logは成功したCargoのraw outputも保存し、parser例外時には該当実行の出力をjob logへ戻す。compiler failure artifactが空でも、shardのraw log・boundary・event fileをfailure artifactへ保存する。この修正の小さなnative fixtureとPython検査は、全workspaceのActions実行成功や性能改善を証明しない。

## Platform contractsへの独立検証の移動（2026-11-02、ローカル実装）

main `2cb7c33aff2ccc3763fa80e6d0f07240ad09570f`（PR #219）のworkflowを基準に、既存`windows-contracts`を`platform-contracts`へ置換する。`ci-plan.outputs.matrix`をそのまま使う1個のjob matrixで、通常の3 OSとL3のmacOS Intelを含む4 OSを維持する。level/matrix producer、`nagi-regression`の二つの広いCargo検査、release/packageの出力契約は変更しない。

| 移す単位 | 同一runner内の依存と保持する境界 |
| --- | --- |
| SF06 signature fake検査、public HTTP client、native artifact、signature分類とartifact（5 step） | fake検査→public client→同じ`RUNNER_TEMP`のlog/native evidence→分類。`steps.sf06_public_client.outputs.closed_failure_diagnostic`を使うartifactも一緒に移動。Cargoが自分のtest binaryと`CARGO_BIN_EXE`をbuildするためrelease binaryを要しない |
| Windows stalled-send/join（1 step） | locked Cargoと2件ちょうどの成功照合。Windows条件、case名、summary、logを維持 |
| Session runtime、auth-stage capture/artifact、deadline capture/artifact、末尾の強制判定2件（7 step） | runner内のraw/sanitized logsと`auth_stage_diag`/`session_deadline_diag` outcomeを同じjobに維持。`continue-on-error`、`always()`、TMP/TEMP検査、bounded capture、cleanup、cfgと強制失敗を変更しない |
| SF01 observer/policy/leaseとHost authority（4 step） | Cargo自身のtest/native compiler。packageの非Windowsコピーを除き、既存Windows側の同一bodyを全platform各1回にする |
| 既存Windows native-send/private Session（2 step、移動元は既存Windows job） | Windows条件、30分budget、source SHA、3件照合、TMP/TEMP、private diagnosticをそのまま維持 |

SF07 checkpoint/prelogin observer/artifact、B1 observer/acceptance、BrowserLogin business witnessはpackageに残す。release compiler build、archive/distribution検証、PowerShell 5.1/7 demo、High/Low applications、installer/upgrade/uninstallerとUser PATHは実際のrelease成果物を使うため同じpackage runnerに残す。移動する単位には`GITHUB_ENV`の隠れた前段依存はなく、SF06 step outputとSession outcomeはjob境界を越えない。

新cacheは`nagi-platform-contracts-v1`専用で、platform/OS/architecture、toolchain、Cargo lock、workflow/diagnostic source、manifest/compiler/runtime/runtime-core source、およびdefault/auth/browser/SF01 oracle/SF06 debug無効設定のversion付きconfig setをkeyへ含める。`target`と`native-target`を保持し、package/regression cacheへのfallbackを設けない。全job成功後のmain pushだけ保存する。元のinline/step `RUSTFLAGS`を維持し、空の`CARGO_ENCODED_RUSTFLAGS`によるdiagnostic cfgの上書きを追加しない。

Readyと公開前の独立gateは新job IDを要求する。選択時は`success`だけ、非選択時は`skipped`だけを許可し、欠落、失敗、取消、未知/partial result、縮小・重複・不正matrixを拒否する。GitHubのmatrix集約resultと固定した全matrixに依存しており、各platform独自の実行receiptを新設したわけではない。元々の検証群の件数・名前照合とfailure artifactsを保持する。

新しい最外周inverseは基準workflow SHA256 `127de03b03d887ea04265454409568f39d3ab8a754c1a511db293bd0388b62f0`へbyte単位で戻る。歴史hash/action pin/Windows/cache proofを更新せず合成する。移動漏れ・重複・順序・body/OS条件/cfg/count/source pin/cache/failure artifact/matrix/required-jobのmutationを拒否する。新規テストを含む`python -B -m unittest discover -s scripts/ci -p 'test_*.py'`は254件成功。移動対象の診断fake/harness 8本も計201件成功（Cargo/native起動なし）。YAML parse、全明示bash stepの`bash -n`、inline Python heredoc 34件parseも成功した。Rust/Go/native/実4 OS runnersは実行しておらず、独立最終reviewとexact-source CIは次のgateである。Session 504の未知の原因を直した変更ではない。

基準#219（run `41449198678`、exact source `7a05a1d`、merge後上記baseと同tree）は全選択検査とReadyが成功し、Readyは13:42:20、全体は42分28秒、Windows packageは39分15秒だった。Session auth 4分52秒、SF06 client 4分35秒、stalled-send/join 2分30秒の観測合計は11分57秒で、package laneから移る観測step時間であり短縮予測ではない。部分観測のSession runtime 1分28秒を完了時間として加えない。distribution 3分26秒、High/Low apps 2分53秒、HTTP registry 2分35秒、PowerShell demo各約2分、release build 1分39秒等はpackageに残る。Linux集約26分34秒後の待ちと全Linux shard cache missも記録されている。

期待wallは`max(package残り, platform-contracts, regression, 他の必須job)`に支配される。warm cacheで移動分の重なりが増える場合、cold compile/setupが相殺する場合、regression/I/Oが次のtailになる場合の三つを区別する。job数は通常+2、L3は+3 runnerになり、compile/setupの重複によりrunner-minutesが増える可能性がある。基準#219は279.47 runner-minutes、旧#217は250.42だが、source/cache/失敗refが違うため性能差の証明にはしない。変更後は同source/選択matrixでcold/warmを分け、queue/依存待ち、setup、cache hit/keyとrestore/save、compile、test、artifact、package、Readyと全体wall、runner-minutesを別々に測る。今回のローカル配線検査を速度改善の実測として扱わない。

## Platform contracts profile実験（2026-11-03、#220確認済みHEADからのlocal変更）

確認済みHEAD `838143426d5171f9dcf30b5f8faced4d5cc7fabe`から分けたbranchで、既存`platform-contracts` jobの全選択matrixに`CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_INCREMENTAL=0`を設定する。debug設定は`compiler/tests/support/native_triple.rs`、`http_client_native_runtime.rs`のbounded native build helperと既存SF06 stepに揃える。Cargo profileのdebug informationを無効にするだけで、最適化level、`debug_assertions`、test選択、commandは変更しない。Rust 1.99.1での`rustc --print cfg -C debuginfo=0`と`-C debuginfo=2`は同じ出力で、`debug_assertions`も含まれる。この確認はcfg設定についてだけで、compile性能やcache sizeの証拠ではない。workspaceのdev/test profileは既にincremental無効であり、job envはchild Cargoも一貫させる。Nagi独自の`--incremental`生成body・receipt検査は同じsuiteに残る。

cacheは明示的なprofile/config-set identityと旧keyへのfallbackなしの`nagi-platform-contracts-v2`へ移す。`target`と`native-target`は両方保持する。新しい外側projectionのinverseはこの変更を外して、旧来のfrozen inverseへ渡す前に確認済み#220 workflowをbyte単位で復元する。旧command hashや#220 branchは変更しない。command、count assertion、source digest、diagnostic、artifact、4 platform matrix、Ready/publication gate、main-only cache save policyも変えない。

#220の基準run 41568477100は全checkとReadyが成功し35:23、Windows `platform-contracts`はcache missで32:54だった。最初のWindows send-deadline stepでは3件のtest実行0.10秒に対してcompileが188秒だった。これはcold compile調査の根拠であり、このprofile変更が時間やcache sizeを減らす証明ではない。候補の変更後Actions実行は未測定。CPU、archive size/transfer、cold/warm compile、job latency、Ready wall、runner-minutesはexact-sourceの比較runまで不明のまま残す。広いdiagnostics分割はmatrix job、cache role、setup/compile費用が増える別案として保留する。
