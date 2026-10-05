# コンパイラ・Rust境界の進捗

## PR0: 設計監査と段階計画

2026-10-05。基点main `8f6cc6cf7d7c08811736325263618cbea19314b8`。PR #76はユーザー側でマージ済みで、head `13b59aa`とmainのtreeが一致することを読み戻した。

### 実施内容

- 依頼全文、AGENTS、DESIGN日英、invariants、pipeline、ADR 001〜005、#76結果と関連実装・テストを照合した。
- Sol 2人が、Phase 1のchecked/provenance境界と、Phase 2〜4のbuild/resource/DBを分担して読み取り監査した。監査だけで成功を主張せず、根拠ファイルと未検証範囲を記録した。
- [全体計画](compiler-rust-boundary-plan.md)にGuarantee Register、owner、checked入力、provenance、error分類、各Phaseのacceptance・互換性・性能・Stopをまとめた。
- [Q-001](open-questions.md#q-001-同じアプリの識別と実行ファイルの世代を分ける)に、既存binary path期待とgeneration isolationの衝突、選択肢、推奨移行案を記録した。
- compiler/runtime、依存、生成ファイル、既存test期待、CI設定は変更していない。版更新・merge・releaseも行っていない。

### Acceptanceと検証

設計・実装状況・予定保証を区別する。PR0は文書のみなので、新しいcompiler conformanceの成功は主張しない。

- `python -m unittest discover -s scripts/ci -p 'test_*.py'`: 51成功。
- website build: 既存website用venvで90ページを生成し、local links/anchors/assetsを検証した。通常Pythonにはmarkdown-itがなく失敗したため、既存venvを使用した。出力はbuilderが許可する`build/boundary-plan-site`へ置いた。
- 変更7文書の相対リンク・anchorは171件を確認し、欠落なし。`git diff --check`も成功。
- [PR #77](https://github.com/disnana/Nagi/pull/77)の初回head `06c20ca`は[checks run 37300208409](https://github.com/disnana/Nagi/actions/runs/37300208409)・[website run 37300207891](https://github.com/disnana/Nagi/actions/runs/37300207891)が成功。PRのDocs-only比較でRust/native/editor/releaseはskip、change detection・release plan・merge gate・siteが成功。skipを新たなRust検証として数えない。
- Q-001承認を反映したhead `b156e05`も、[checks run 37300934374](https://github.com/disnana/Nagi/actions/runs/37300934374)・[website run 37300933963](https://github.com/disnana/Nagi/actions/runs/37300933963)が成功。更新pushのchecks/siteも成功を読み戻した。
- 新branchの初回pushは比較基点がなく、既存fail-safeによりLinux全suiteも起動した。PRの文書差分判定とは別で、これを新しいcompiler変更の検証と取り違えない。
- compiler/runtime/依存の変更がないため、今回の文書確認をRust build・4 OS・runtimeの新しい保証に数えない。

### 新しい反例とGuarantee Registerへの影響

新たなNagiプログラムのcheck/build反例を実行して発見したフェーズではない。見つかったのは、`shared_target.rs`がapp identityとgeneration pathを同一視する既存期待と、新依頼の契約の衝突である。

Guarantee Registerの「現在」は既存のownerを維持する。G-SEALED/G-GENERATION/G-TXは予定で、まだ保証していない。Auth ScopeはPhase 5以降の方向のみ。Rustへの最終borrow/trait/Send/Sync委譲は変更しない。

### 未解決事項・次Phase

Q-001はユーザーがAを承認した。app identity維持、generation別のside-by-side生成、build成功後のatomic latest更新を採用する。旧generationはbuild時に上書き・削除・killしない。承認済み設計に伴う内部path等のtestは理由を記録して更新できる。公開意味論・利用者契約・High/Low・登録保証・security/lifecycleの期待変更は引き続きStop。

計画とworking rulesへ反映済み。PR0の更新CI成功を確認し、Phase 1のfailing testsから再開した。Phase 2〜4を同時に実装しない。

その後ユーザーが#77をmainへマージした。main `ded4c44cd3ebf984b382995322cefc769b4a6cb3`のtreeは承認済みhead `b156e05`と一致することを読み戻した。Phase 1のPRはこのmainをbaseにする。

## Phase 1: 最終check済み入力の封印

Phase 1のacceptanceとCIは完了した。PR0とは別branch `refactor/checked-program-boundary`で実施し、その後ユーザーがPR #78をmainへマージした。エージェントはマージ操作を行っていない。版更新・releaseは行っていない。

### 実装前の観測

`emit::rust(&Program)`の禁止を表すcompile-fail testを先に追加した。旧APIではコンパイルが成功し、`cargo test --locked -p nagic --doc`が「compile-failがコンパイルできてしまった」と失敗した。commit `3f76c2b`に保存した。この失敗をsealed APIで解消する。

既存callerは最終factoryへ移行する。内部factsを故意に破損するテストだけは再checkさせず、欠落・改変を検知するoracleを保つ。通常fixtureのUser Low扱いは既存生成比較のbridgeであり、実ファイルprovenanceの検証とは分ける。

[ADR 006](adr/006-sealed-codegen-input.md)に採用・不採用・保持する意味論を記録した。

### ローカル検証

最終factoryと封印APIを実装し、生成側のcapability/view/storage等の判断を封印時へ移した。既存fixtureはfactoryを通すhelperへ移行し、assertは維持した。内部破損のnegative oracleだけは再checkを行わない。

全suiteは89 suite・784成功、clippyも成功。SQL engineなし8成功、256生成caseを2つのseedで検査、10,000 mutation/128 native caseも成功。VS Code 196、HTML viewport 5成功。旧版の17正例から得たLow/直接Rust/保存Low Rustの51ファイルはbyte一致した。

最初のHTTP実行は通信制限によるloopback bind失敗、最初のeditor実行はcompilerのPATH未設定で失敗した。同じテストを必要な環境で再実行し、期待を弱めず成功した。詳細・発見した封印の穴・性能条件・保証の限界は[結果](checked-program-results.md)に記録する。

[PR #78](https://github.com/disnana/Nagi/pull/78)をmain向けに作成した。最初のCIではcompiler変更によるLinux/JetBrainsが起動したが、4 OS配布検証がskipされた。release planが版更新と配布設定だけを条件にしていたためで、成功とは数えない。compiler/runtime/Cargo入力にもNagi配布検証を適用する回帰と条件を追加する。版が変わらないときに公開しない規則は維持する。

更新head `08199bf2758b7c09688a05a80527566dc2a03e6c`の[checks run 37308375211](https://github.com/disnana/Nagi/actions/runs/37308375211)が成功した。Linux全suite、Windows x64、Linux x64、macOS Intel/Apple Silicon、VSIX package、IntelliJ IDEA、PyCharm、merge gateを読み戻した。[website run 37308374524](https://github.com/disnana/Nagi/actions/runs/37308374524)も成功。publish-releaseはskipで、公開したとは報告しない。

Phase 1のacceptanceを満たした。ユーザーによる#78のマージを読み戻し、main/merge SHA `0107f37f0de6026533a4d67f52c6545054b71584`、tree `87b8b6ec8d758b111eec4725e5746e5560d9b5a5`を確認した。merge commitはhead `08199bf`を親に含み、treeも一致する。G-SEALEDはmainへ反映済みで、正式releaseは未実施。Rustへの委譲やruntime意味論は維持した。エージェントはmerge・版更新・releaseを行っていない。

## Phase 2: build generation isolation

Phase 1のCI成功後、`fix/build-generation-isolation`へ分けて着手した。作業基点は#78のhead `08199bf`。その後#78のmain反映を確認したため、Phase 2のPRはmain向けとし、Phase 2の追加差分を比較する。Phase 2はmain未反映で、こちらではマージしない。

[ADR 007](adr/007-build-generations.md)へ、常設canonical out lock、app IDとgeneration、孤児Cargoを含むbin分離、成功時latest、互換projection/lock/cache、維持する負例と測定を実装前に記録した。G-GENERATIONはこのbranchで実装・ローカル検証済み。acceptanceとCIの完了はまだ主張しない。

### 先行回帰の初回観測

Rust実装を変更する前に、実Cargoのgeneration回帰13件は2成功・11失敗、成功artifactを選ぶPython helperの回帰5件は1成功・4失敗だった。同一outの後続Cargoの侵入、writing checkの先行完了、孤児Cargoが可変projectionを読むこと、固定cache exeの上書きを観測した。metadata関連の負例は、成功metadataが存在しない旧実装で失敗した。これらを「新実装が検証済み」とは扱わない。

Solによる独立レビューで、marker不在の時間待ちだけではlockを証明できないこと、process期限・孤児Cargo成功・最新metadata置換失敗・snapshot内容のoracle不足を指摘した。OS lockの競合と待機通知を正のbarrierにし、失敗テストを補強してからtests-only commitへ残す。Windowsの既存latestを削除共有なしで開く負例は、temp作成失敗とは分ける。

旧CLIのexeをcopyしてhashを記録し、専用の空cacheで小さなstd-only appを15回build/runした。空runtimeを使い依存frameworkの時間を除いた測定で、空cache1回は198.003ms、同じ内容7回の中央値は77.528ms、変更あり7回は73.913msだった。変更後も同じ条件で比較する。runtime throughputやallocationの測定とは扱わない。

補強後のtests-onlyを固定した。Linuxのgeneration回帰14件は2成功・12失敗、既定/明示shared targetは2失敗、project cwdの実Cargo回帰は1失敗、Python6件は1成功・5失敗。既存CLI7件とinput保護10件は成功を維持した。project負例はstdout・cwd・cacheを通過し、新generation未実装の箇所で失敗した。Windows専用のlatest置換失敗はこのhostでは未実行で、4 OS CIへ組み込んだ。source実装差分がない状態のtest hashと失敗ログを保存した。

### 実装中のレビューと回帰

生成の可変cacheと成功artifactを分離し、`BuildGeneration::finish(self, …)`が公開・互換出力・latest更新の順序を固定する。input保護はlock取得前後で確認する。Lowの非書込みcheck/lowerはlock待機を増やさない。

独立したSolレビューで、stagingから公開先へ移るとruntime相対参照が壊れることと、互換出力が旧latestへのhard linkなら旧metadataを破壊することを確認した。元の実装で失敗する回帰を追加してから、同親・同深さの公開と同じ出力集合での保護へ修正した。Rootレビューで同じalias問題がwriting check/lower/costにも残ると確認し、writer共通の保護へ広げた。

旧CLIで受理される非UTF-8 cwdと200文字のsource basenameも、新実装で拒否されることを実行で確認した。OS pathの生unitsをsnapshotへ残し、bin名を固定長app hash＋generationへ短くした。app IDとpackage名は維持する。raw非UTF-8 argvが`std::env::args`でpanicする既存P2は、旧CLIでも再現するためこの内部path移行には含めない。

既存output保護のmanifest比較は、承認済みの世代bin名だけを正規化し、他の全keyとLow/Rust/input bytesの一致を維持した。Linuxの最終対象回帰はgeneration 23、CLI 7、output保護10、shared target 2が成功し、clippyも成功した。project smokeは実runtime依存をbuildしてstdout/cwd/相対設定を確認した。

故意に親を終了したwrapperは、statusとartifactから処理完了を観測する。wrapperが起動した直接のCargoは期限内にwaitするが、OSに引き取られたwrapperのreapまでtestが保証するとは書かない。subreaperやunsafeは追加しない。

### ローカル検証の完了・CI待ち

`cargo test --locked`の最終再実行は90 suite・807成功、失敗・ignoreなし。fmt/clippy、2 seedの拡大conformance、10,000 mutation/128 bounded native、SQL engineなし8、Node 201、Python native helper 9・CI判定51・release scripts 90、application 10 project/19 runも成功した。Docsは90ページを生成し検証した。詳細・失敗からの再検査・保証の限界は[Phase 2結果](build-generations-results.md)に記録する。

初回全suiteは`typed_errors`の旧内部cache path assertが1件失敗し、その後7件がmutex poisonで失敗した。Q-001に基づき成功metadataのnamespaceとcache bytesの一致を検査するassertへ更新し、対象15件と全suiteを再実行した。Node初回のchild spawn `EPERM`も記録し、同じテストを必要なpermissionで再実行して201成功を確認した。期待の緩和や失敗のskipは行っていない。

freeze時の独立レビューではowned 12ファイルのhash一致を確認した。その後、Python metadata readerとそのtestだけに、top-level object・整数schema versionの厳密な検査を追加した。修正前は9 test中2 failure・3 errorを記録し、修正後9件が成功。Rust sourceのfreezeは維持した。

旧・新CLIで合計90 build/runを測定し、生成Rustは45 pairすべてbyte一致した。[測定summary](../../benchmarks/results/build-generations-2026-10-05/summary.json)では初回・repeatのwarm中央値が増え、交互測定はほぼ同程度だった。速度不変・高速化・因果的なoverhead上限は保証しない。binaryはこのfixtureで120 bytes増えた。以前のFuture frame +32 bytesは未解決で、今回の測定では再検査していない。

Windowsを含む4 OS CIは未完了。Phase 2のacceptanceは再確認待ちを維持し、Phase 3のcharacterization/refactorは未実装。Phase 2のmainへのmerge、版更新、releaseは行っていない。

PR #79をmain向けに作成した。初回CIではmacOS Apple Siliconのgeneration 22件が成功したが、非UTF-8名のfixture作成がNagi起動前にAPFSのOS92で失敗した。Linuxの元の回帰を維持して対象OSを修正し、共通の日本語pathでsource/provenanceの生OS unitsを検査するassertを追加した。[結果](build-generations-results.md#初回ciでのfixture修正)に理由を記録した。4 OS CIの再確認までacceptance待ちを維持する。

### 修正後のacceptance

head `27c8bf4`の[checks run 37330160221・attempt 2](https://github.com/disnana/Nagi/actions/runs/37330160221)と[website run 37330159710](https://github.com/disnana/Nagi/actions/runs/37330159710)が成功した。Linux全検査、4 OS配布・10 project/19 run、VSIX、IntelliJ IDEA、PyCharm、merge gateを読み戻した。generation回帰はLinux 23、Windows/macOS各22件。Windows latest置換失敗・回復も成功した。旧headイベントの重複で取消されたattempt 1を成功とは数えない。詳細は[結果](build-generations-results.md#修正後のci)へ追記した。

Phase 2のacceptanceを満たし、#79をreview可能へ変更した。mainへのmerge・版更新・releaseは実行していない。次段階は#79の成功headを基点に進め、#79がmain未反映の間は依存と最終反映先mainを明記する。

## Phase 3: 登録資源の契約と先行characterization

branch `refactor/resource-contract-foundation`、基点は#79のhead `27c8bf4`。[ADR 008](adr/008-resource-contracts.md)に、公開ResourceInfoを内包する単一descriptor、分類集合から導くgeneric role、用途別legacy query、lifecycle不活性、維持する受理・拒否を記録した。集約実装はまだ変更していない。

2つのSolレビューで、Passing inventoryがtype_parametersを読んでいた誤り、Requestのis_* fieldの省略、Borrow/Mapperのgolden coverage不足を訂正した。全22resource・47operation・32fieldをコードへ照合した。新targetの4 OS明示一覧への追加、harness登録と実行の区別、runtimeが必要なclassをstandalone rustc corpusへ入れない条件も先行案へ反映した。

goldenは実resolverへ固定logical identityを渡すcfg(test) fixtureを使い、metadata・alias・deriveを削らず比較する案を採用する。既存物理fileのHigh/保存Low一致・native・診断位置は維持する。先行test-only commitのCI成功後にだけ集約へ進む。

Copy深さ63/64/65を4種類のleafで検査した。checkerが2回使用を拒否し、生成型はCopyになる差を確認した。owned/Optionの深さ65では、手書きadapterのCopy要求だけがE0277になり、同じNagiを要求なしの別adapterでbuildすると成功した。今回の有限probeではNagiだけのaccepted-invalid、unsoundnessは確認していない。P2の二重判定として[調査](copy-boundary-investigation.md)へ原因・matrix・再現生成器・判断案を残し、正常golden・skip・allowlistへ固定しない。

先行test-onlyをfreezeした。新規13件、対象125件、全suiteは91 suite・820成功。fmt/clippy、Python CI 52/release 90、site 90ページ、38 corpus/16harnessの登録確認も成功。別のSolが全文golden・独立期待・旧assert・production不変をレビューした。詳細と初回oracle/capture失敗の区別は[結果](resource-contract-results.md)に記録する。4 OS CIの成功前に集約実装を開始しない。
