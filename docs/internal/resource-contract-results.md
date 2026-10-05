# 登録資源の契約: 先行テストの結果

2026-10-05。Phase 2の成功head `27c8bf4`を基点にしたPhase 3。採用設計は[ADR 008](adr/008-resource-contracts.md)。先行test-onlyのhead `eb93873`で4 OS CIが成功した。以下は集約前の観測であり、本実装の完了結果は分けて追記する。

## 独立した期待と生成例

- [`resource_contract_characterization.rs`](../../compiler/tests/resource_contract_characterization.rs): 22resource、47operation、32 accessorと全constants。署名、generic/value arity、Passing、borrow_owner、集合完全性、ResourceInfoの公開fieldと戻り値形を独立した手書き期待で確認する。
- [`capabilities`の用途別test](../../compiler/src/capabilities.rs): inline/shared payload、Actorの間接protocol、callback signature、nominal phantomを区別する。native Debug、Serde、Charge、owned field storageを同じ条件にまとめない。
- [`resource_contract_goldens.rs`](../../compiler/src/tests/resource_contract_goldens.rs): HTTP検査、Borrow/Mapper、Actor data、data deriveの4例。実resolverへ固定logical ModuleIdを渡し、High check→Low→独立finalizeと直接finalize→Rustを通す。metadata・symbol・alias・deriveを削らず、8ファイル計99,254 bytesを比較する。
- [`http_codegen.rs`](../../compiler/tests/http_codegen.rs): 既存実fileのHigh/独立Low比較を維持。追加Cargo例は非Copyの文字列DTOをJSONへ借用し、App/routeへnamed mapperを登録する。mapper本体やHTTP dispatchの実行を新しい保証にはしない。

固定logical IDのgoldenは実fileのprovenanceを保証するものではない。元位置・実HTTP・Actor・共有field・owned Copyのnative検査は既存harnessを維持する。旧HTTP/Actor sourceは抽出前の1,241/245 bytesと一致し、既存assertは削除していない。

## 先行検査中の失敗

4goldenは最初に空の期待値との差で失敗し、実生成結果を採取して固定した。採取用の一時コードは撤去し、通常testから期待fileを更新する仕組みは残していない。期待8ファイルは採取bytes・SHA-256と一致する。これは新機能のfailing testではなく、現行生成のcharacterizationである。

手書き定数oracleの初回ではCallKind.NOT_READYの転記漏れ1件を検出した。既存登録を再確認し、oracleだけを訂正した。production、既存test期待、受理・拒否は変更していない。最初のclippyのtuple complexityはtest用の型aliasで解消し、allowを追加していない。

CI配線の回帰は、4 OSの明示一覧に新inventoryと既存shared-field testがない状態で失敗した。一覧へ追加後、選択を確認するPython testが成功した。版変更がないときの非公開規則は維持する。

## ローカル検証

- 対象125成功: library 113、inventory 4、HTTP codegen 4、Actor codegen 2、owned Copy 2。新規は13件。関連nativeのHigh/保存Low計6 runも成功した。
- `cargo test --locked`: 91 suite・820成功、失敗・ignoreなし。
- fmt/clippy all-targets: 成功。
- CI判定のPython 52、release scriptのPython 90: 成功。release testsは模擬入力であり、実際の公開ではない。
- conformance登録検査: corpus 38、linked harness 16。登録確認を実行成功には数えず、対応Cargo testの実行は上の結果とCIで確認する。
- website: 90ページを生成し、links/anchors/assetsを確認した。

Solが登録期待と生成testを実装し、別のSolがfreeze差分を独立レビューした。rootも旧入力・production不変・hash一致・全suiteを確認した。レビュー自体を言語の完全性の証明にはしない。生ログとhash証跡は作業環境の`/workspace/test-tools/compiler-rust-boundary-plan/phase3-test-logs/`と`phase3-test-only-full.log`に保存した。

## CI・残課題

先行test-onlyは[PR #80](https://github.com/disnana/Nagi/pull/80)に保存した。初回head `ca9362e5`の[checks run 37338704930](https://github.com/disnana/Nagi/actions/runs/37338704930)で、macOS Apple Siliconの既存shared-target testが失敗した。新inventory・golden・HTTP nativeは同jobで成功しているが、job全体の成功とは数えない。4 OSの完了を確認するまでResourceContractの集約を実装しない。

失敗は、もう一方のtest成功直後にCargoがcurrent directoryを見失ったもの。同時刻で2つのFixtureを作る小さい回帰では、旧factoryが同じdirectoryを借用し、一方のDropが他方のsentinelを消すことを再現した。P2のtest資源所有権の欠陥は確定。CIの個々の時刻衝突までtraceした証拠はないため、その失敗との因果は整合する候補として残す。

fixtureへprocess内AtomicU64の識別子とexclusive create_dirを追加した。承認済みの内部test pathの修正で、既存2件の言語・世代・cache・実行assertはすべて維持した。同tick回帰と旧2件の3成功、targeted clippy/rustfmt成功を確認。skip、retry、test全体の直列化は追加していない。上記820件の全suiteはこのfixture修正前のローカル結果。修正後のLinux CIの全suiteは91 suite・821成功で、追加回帰1件を含む。

### 先行test-onlyのCI acceptance

head `eb93873a9a1f2583f2be09eff8028941a19399e9`、tree `fd09e4e9f71440bb37fbca84735f65bb994d8cef`の[checks run 37341673174](https://github.com/disnana/Nagi/actions/runs/37341673174)・attempt 1が成功した。Linux全検査、4 OSの配布・実application・インストール検証、VSIX、IntelliJ IDEA、PyCharm、merge gateを読み戻した。[website run 37341672775](https://github.com/disnana/Nagi/actions/runs/37341672775)も成功。publish-releaseはskipで、公開はしていない。

4 OSそれぞれの完了ログで、inventory 4件、全文golden 4件、用途別3件、同tick fixture回帰、借用JSON・named mapper登録のnative回帰の実行成功を確認した。harnessの登録だけから成功を推測していない。先行テストのacceptanceを満たしたため、同じ期待を維持する集約実装へ進む。これは集約後のCI成功ではない。

PID/時刻を使う他21 fileも読み取りで確認した。atomicを持たないものは11 fileだが、prefix・単発実行・exclusive作成等の条件が異なるため、すべて同じ欠陥とは断定しない。integer_arithmetic、integer_zero_division、conformanceのfactoryはP2候補として、同tick注入の再現と共通allocatorの適用を次の監査対象にする。未再現の候補を成功や修正済みには数えない。今回の資源集約前に広範なtest rewriteは行わない。

#79未マージの間はstacked PRで依存を明記し、最終反映先はmainとする。こちらではmerge・版更新・releaseを行わない。

[Copy深さの差](copy-boundary-investigation.md)は別のP2として記録した。今回のinventory集約で受理・deriveを変えたり、差を正常goldenへ固定したりしない。Pool/Tx/lifecycle・新しい保証は未実装。既存Rustへのtrait/Send/Sync/link/依存環境の委譲も維持する。

## 集約実装

production差分は`stdlib.rs`と`capabilities.rs`。22個のnamed static ResourceContractへ既存ResourceInfoを移し、公開queryはその参照を返す。shared payloadの旧queryとnative Serde拒否は同じdescriptorを使う。型引数の5種類のroleは登録sliceから導き、独立したrole配列や用途共通のwalkerは追加しない。

各staticのconst constructorで、generic arity、全indexの範囲、各positionがちょうど1つのroleに属することを検査する。通常lookup時のregistry走査はしない。lifecycleはUnspecified、現資源のSerdeはfalseのまま。operation、checker、emitter、runtime、依存版・featureは変更していない。

private unit testを8件追加した。旧queryが同じdescriptorを参照することと手書きrole期待の正例2件、欠落・同一slice内の重複・role間の重複・範囲外・arity 0へのrole・type parameter数不一致の負例6件。先行inventoryと全文goldenの期待値は変更していない。追加testも既存linked library harnessへ登録した。

対象205件、fmt/all-target clippy、CLI buildは成功した。4goldenのLow/Rust全文、元のNagi/Low入力、public ResourceInfo shape、全22登録値、operation・accessor・constantの内容を維持した。checker/checked/ast/emitter/runtimeは変更していない。全suite、bounded生成探索、4 OS CIの完了は分けて追記する。この節だけでacceptance済みとはしない。

### frontend測定

同じphysical input/cwd/output pathで、4fixtureのcheck/lowerを各warmup 1＋測定7回実行した。前後計128起動が成功し、全反復と前後の未正規化Low bytesが一致した。[環境・生データ・比較](../../benchmarks/results/resource-contracts-2026-10-05/README.md)を保存した。中央値には増減があり、共有hostでのこの小さい観測から速度不変や高速化を保証しない。timerはprocess起動・frontend・CLI filesystem処理・pipe収集を含み、Cargo/runtime throughput/allocation/RSS/CPU/Future sizeは測っていない。

Docs生成の最初の試行は、scriptの出力制限に反する`/tmp`指定を拒否された。productionを変えず、対応する`build/`内で再実行して90ページ・links/anchors/assets検査が成功した。初回失敗を正常な生成結果とは数えない。

### 集約後のローカルacceptance

- `cargo test --locked`: 91 suite・829成功、失敗・ignoreなし。先行test-only修正後821件にprivate unit 8件を追加した。
- 対象205件、fmt/all-target clippy `-D warnings`、CLI build: 成功。
- seed `305419896`と`3735928559`で各256生成case＋固定38corpus。両seedともconformanceの全5testが成功した。
- fuzz smoke: 10,000 mutation、parse拒否7,149、check拒否1,910、受理後Low/emit成功941、panic 0、bounded native 128。coverage-guided fuzzや全受理プログラムの証明ではない。
- 独立Solレビューとroot照合: 旧22 ResourceInfo値、公開shape、operation/identity/accessor/constants、目的別判定順、fixture/golden期待の維持を確認。今回の差分に新P0/P1/P2は見つからなかった。既知Copy差や他のfixture候補が解消したとはしない。

SQL engine無効検査の初回は存在しないtest targetを指定してCargoが拒否した。conformance/fuzzの成功と区別し、CIと同じbuild＋cli/sql_check commandで確認し直し、buildと8件が成功した。4 OSの集約後CIは別に確認する。

### 集約後CI: WindowsのAxumサンプルで停止

head `0b2a5a5805481b716d789061114e003adf68704d`、tree `41c766800d379f552d59d3fc6c3b333e3efd0f11`の[checks run 37347188897](https://github.com/disnana/Nagi/actions/runs/37347188897)は失敗した。Linux全検査とLinux/macOS 2種類の配布検証、VSIX、IntelliJ IDEA、PyCharmは成功。[website run 37347187618](https://github.com/disnana/Nagi/actions/runs/37347187618)も成功した。4 OSの実ログで新しいprivate unit 8件の成功を確認したが、Windows job全体とmerge gateは失敗している。publish-releaseはskip。Phase 3のacceptanceは未達で、#80はdraft、Phase 4は未実装のままとする。

Windowsの失敗は`axum-service`の保存Low実行で、Content-Typeのない13バイトPOSTに対する415を読む前の`WinError 10053`。同jobのHigh実行は成功した。元のrequest、415期待、本文検査を残し、retry・skip・一括送信への置換は行っていない。

一次コードでは、AxumのJson extractorがContent-Typeの不適合を本文読取より先に拒否し、Hyperが未読本文を一度pollして残っていればreadを閉じる経路を確認した。Pythonの通常requestはheaderとbodyを別々にsendする。既存High/Low executableを使ったLinuxの固定各24観測では、本文をまだ送らなくても415とEOFまで届いた。Windowsの10053自体は再現しておらず、その直接原因を確定した証拠ではない。

これは資源descriptorの値・生成byte一致を破った証拠ではないが、CI失敗を無関係として除外もしない。通常の合法requestを一括送信へ変えるだけでは元の配送条件を失うため採用しない。sample adapterの期限付き本文読取は新policyになるので、上限・期限・415の優先・close条件を具体化して判断する。本体runtimeやAxum/Hyperの第三者sourceは変更しない。

生ログ・job/step結果・8件の実行行・source hash・Linux観測は作業環境の`/workspace/test-tools/compiler-rust-boundary-plan/phase3-production-ci-failure-proof.json`と`axum-early-rejection-*`へ保存した。調査と修正後CIの成功を混同しない。
