# コンパイラ・Rust境界の進捗

## 2026-10-08: Security Foundationの調査/RFC

現在mainは#99 merge後の`62bbda9`、SQLite公開APIを含む4 OS/Linux/site CIをreadback済み。公開0.1.11は`003a594`でSQLite公開API未収録。確認時open PR0。過去のdraft/pending記述は履歴で、最新状態はこのmainとGitHubを正とする。

[Security Foundation RFC日英](security-foundation/rfc.md)と[PR/検証/全体完了計画](security-foundation/implementation-plan.md)、[source根拠付き調査](security-foundation/baseline-audit.md)を作成した。AuthScope/CSRF/XSS/SQL Injection/SSRF/CORS/Cookie/Session/DoSの予定契約を現行保証と区別する。D1–D3の公開選択は未採用、新API/compiler/runtime実装は未着手。独立reviewの指摘/修正/再確認とDocs検査は[review台帳](security-foundation/review-log.md)、次順序は[引継ぎ](handoffs/2026-10-08-security-foundation-rfc.md)へ記録する。merge/版更新/tag/正式releaseは行わない。

## 2026-10-06: S1 main反映とS2サービス接続

PR #88最終head `08e90c6`は4 OS・必須CI成功、未解決review0でmain `aee1987`へmergeした。両tree `bc62c787471d9e4481e36c0cca6e2ed294cebbc1`を読戻した。S2のPR #90は既存Task awaitの内側Resultを親tryへ接続し、HTTP旧spawnを維持する。新API/意味論/compiler/runtime/依存は追加していない。native三構文×7終了ケース、公開両例の三構文native、独立Sol High指摘3点の修正と読戻し、全回帰と4 OSを完了した。最終head `f1497053`のchecks `37545273020`・website `37545272936`が成功し、main `f9b25782`へmerge、共通tree `84d1696053a9cf5ef256b44fcd50afff91863c9d`を確認した。[S2結果](task-handles-s2-results.md)が正本。次は別PRの0.1.11版更新・移行資料・独立最終review・公開検証で、[release引継ぎ](handoffs/2026-10-06-task-release-0.1.11.md)へ記録する。旧節の停止・未merge指示は当時の履歴で、最新のリリース完了指示を制限しない。

agent運用はPR #89で整理しmain `f65c6093`へ反映、Fast Luna Max、Engineer/Reviewer Sol High、Architect xHigh、Critical Max、Astra例外をcustom TOMLへ接続した。checks `37544283368`・website `37544282939`成功、tree `4d2faacf5eeea630ab8bae7968fe5a2708f42046`を確認。[構成記録](agent-routing.md)で仕様検査とstandalone CLIの未実行範囲を分ける。Taskのproduction検証成功へ数えない。

## PR #88最終レビューとTask次リリース準備

2026-10-06。#88をS1完成PRとしてレビューし、production本体・依存を維持した。未実装と書かれた日英/AI文書、negative9対＋positive1対、旧Supervisor terminal→実HTTP停止の三構文専用oracleを補強。契約110/110、Task native6群、全workspace原ログ95 block・934成功・failed0・費用用ignored1、fmt/clippyが成功した。過去の90/90・source CIを今回の再実行へ数えない。

[最終レビュー](task-handles-s1-final-review.md)と[完成後引継ぎ](handoffs/2026-10-06-task-handles-s1-complete.md)へ、契約照合・負債・次PR/依存順を記録する。最新ユーザー指示で、S1完成後も停止せずTask残件と次Nagiリリース公開へ継続する。新公開仕様などの判断が不要な実装・検証は自律進行する。新しいSol独立レビューはDocs修正と配布Task三構文/12拒否を確認し、productionの追加不具合なし。release unit98と検証用Linux archive全体も成功した。最新headの4 OS・必須CIを読戻し、既存手順に沿い必要PRのmerge/版更新/releaseまで進める。

## S1 Task結果handleの接続（作業branch・未リリース）

2026-10-06、ユーザーの再開指示を受け、Stage 1の停止境界から追加RED→checker/public runtimeへ接続した。SpawnBind、canonical std.task、stable ScopeIdとbinding義務、sealed受取/放棄/Scope planを実装。High・保存Low・手書きLowの全positive nativeと、業務Err兄弟継続・sticky fault・legacy/body元Err・Drop/actual joinを検証した。独立レビューのFailure wrapper copy/share、spawnユーザー名、associated method再export、raw public checker APIの不一致を縮小反例で修正した。故障後大量受取のO(n²)掃除もRED→ticket退役へ修正し、17native oracleへ追加した。

[接続結果](task-handles-s1-results.md)と[artifact](../../benchmarks/results/task-handles-s1-2026-10-06/README.md)に90/90契約、三構文native、全workspace回帰、探索、測定、日英Docsと独立レビューを記録した。公開source `34ac4d5` の[checks](https://github.com/disnana/Nagi/actions/runs/37489343115)・[website](https://github.com/disnana/Nagi/actions/runs/37489342523)は成功。4 OSともTask native5群・runtime17群・公開3・doc9・checker90/90、Linux全workspace原ログ933成功・failed0・費用用ignored1、全package・両IDE・merge gateが成功した。結果追記headのChecksはPR #88で別に確認する。Stage 1結果を今回の実行と数えず、S2、公開SQLite、merge、release、版更新は行わない。


## S1 Stage 1: 先行REDとprivate Task bridge

2026-10-06。ユーザーによる#87のマージを確認した。最終head `5985e1b`とmain `9ba4a10`のtreeは一致。#87は追加変更せず、S1は別のmain向けdraft PRへ分離する。

[Stage 1結果](task-bridge-stage1-results.md)にprivate runtimeの16群、runner oracleの3群、56入力中6一致/50未達を記録した。新Taskはparse REDのまま。結果通知を実joinとしないこと、受取faultがscope故障を消さないこと、元legacy Errorを保持することをnative oracleで検査し、Solが修正後sourceを独立レビューした。初回socketのinfra失敗と、runner元行/Lowコメント誤りの記録も保存した。

今回は先行RED＋cfg(test) private bridgeで止める。Nagi checker/Low/Rust生成、旧spawnの移行、allocation/Future size測定、公開SQLiteは後続。merge・release・版更新は行わない。全回帰と最新head CIは結果文書へ追記し、過去headの成功を流用しない。最新指示でこの区切りの後に停止し、[Sol 6.1向け引継ぎ](handoffs/2026-10-06-task-bridge-stage1.md)を作成する。checker/public runtimeのTask実装は未着手。source head `6223ad2`の[CI](https://github.com/disnana/Nagi/actions/runs/37467579939)は4 OSのprivate各16群・全必須jobが成功。引継ぎ追記後の最新CIはPR Checksで別に確認する。

## PR #87: 更新後CIの終了回帰と仕上げ

2026-10-06。move補強のlocal `44c38b7`は90 suite・899件成功。公開head `68d215b`のLinux CIでprivate SQLite closeが1件失敗した。古いCI、ローカル成功、再実行だけで解消したとは扱わず、stock APIによる決定的反例をtest-only `0051f7a`へ保存した。

原因はidle Objectと予約済みpermitが同時に存在するとstock closeがsenderを残せること。`cfg(test)` adapterのclose後idle退役5行と回帰1件を追加し、Solが独立レビューした。最終コード `f6bc74a`で全回帰90 suite・900件、failed/ignored 0、fmt・clippy成功。公開runtime、compiler本体、move仕様、依存、CI、版は変更していない。[原因・先行REDと修正](sqlite-close-regression.md)、[仕上げ監査](explicit-move-readiness.md)、[原ログとprovenance](../../benchmarks/results/explicit-move-2026-10-06/readiness/sqlite-close/)を残す。最新headの4 OS・必須CIはPR Checksで別に確認する。

次の実装はS1結果handle→S2業務Err/task fault→公開Pool/Tx。委任に基づく全Tのawait/discardとsticky faultの設計採用は別のlocal設計branchに保存し、#87へ新Task実装を混ぜていない。公開Poolのcapacity allocationブロッカーは未解決。merge/release/版更新は行わない。

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

#80の初回CIはmacOS ARMの既存shared-target fixtureで失敗した。同tickのdirectory共有・他方Dropによる削除を独立した小さい回帰で再現し、atomic識別子とexclusive作成へ修正した。旧2件のassertは維持し、新回帰を含む3件成功、fmt/clippy成功。CIとの因果の確度と同系統の未再現候補は[結果](resource-contract-results.md)に残す。修正後CIの完了前には集約へ進まない。

修正head `eb93873`の[checks run 37341673174](https://github.com/disnana/Nagi/actions/runs/37341673174)・attempt 1とwebsite run `37341672775`が成功した。Linux全suiteは91 suite・821成功。4 OSのログで新inventory/golden/用途別/fixture/native登録テストを確認し、両JetBrains製品、VSIX、merge gateも成功した。publish-releaseはskip。先行test-only acceptanceを満たしたため、同じ期待を保つprivate ResourceContractの集約へ進む。#79はmain未マージ、#80は依存を明記したdraftのままで、集約後のacceptanceとは分ける。

### 集約実装とローカル検証

登録資源22個をprivate named static Contractへ集約した。公開ResourceInfoは内包した既存値の参照、shared/native Serde queryも同じ根拠を使う。constでarity・範囲・重複・欠落を検査し、用途別判定順、Passing、旧の受理・拒否・全文goldenは維持した。productionはstdlib/capabilitiesの2fileだけで、checker/checked/emitter/runtime/依存は変更していない。

private unitは正例2・負例6を追加。対象205件、全91 suite・829件、fmt/clippy、2 seedで各256生成case＋38固定corpus、10,000 mutation/128 native、SQL engineなし8件が成功した。独立レビューとrootも旧値・公開shape・生成bytesの維持を確認した。前後各64回のcheck/lower測定は全成功・Low bytes一致で、中央値には増減がある。条件と生データは[測定](../../benchmarks/results/resource-contracts-2026-10-05/README.md)へ保存した。詳細と初回コマンド失敗は[結果](resource-contract-results.md)に区別する。

集約後の4 OS CIは、このcommit時点では確認前。確認前にPhase 4実装へ進まない。mainへのmerge・版更新・releaseも行っていない。[Pool／Txの具体案](sqlite-pool-proposal.md)と[根拠・代替案](sqlite-pool-research.md)は未採用の資料で、新API/policy/hooksの判断を[Q-002](open-questions.md#q-002-sqlite-pooltxの初版apiと終了policy)に残した。

### 集約後CIの失敗と次の判断

head `0b2a5a5`のchecks run `37347188897`で、Windowsの既存Axum sampleが保存Lowの415受信前に接続abortとなり、merge gateも失敗した。他の3 OS配布、Linux全検査、VSIX、両JetBrains製品、websiteは成功した。資源contractのprivate unit 8件は4 OSとも成功したが、Phase 3完了とは数えない。#80はdraftのまま。[結果](resource-contract-results.md#集約後ci-windowsのaxumサンプルで停止)へ失敗と一次コード・Linux観測の範囲を記録した。

元の通常clientと415期待は維持する。一括sendへ置換してCIの条件を狭める案は採用しない。Axum sampleだけに期限付き本文読取を加える場合は、新policy値・待機・close条件の判断が必要。具体案を作り、承認前には適用しない。Pool／Txは既存Rust pool/workerの再利用も読み取り比較しているが、Phase 4実装は開始していない。mainへのmerge・版更新・releaseも行っていない。

### #79のmain反映とAxum修正の承認

2026-10-06。ユーザーが#79をmainへマージした。main `2f2c93def942e3133eaffbca0ecb596292f95d47`のtree `286b3c0080bc7ce562ab5fdb0612989dd43eb60e`は成功head `27c8bf4`と一致し、そのheadを親に含む。エージェントはmerge操作を行っていない。

同日、ユーザーが[ADR 009](adr/009-axum-rejected-body.md)の案Aを承認した。設計・invariant・Q003をcommitしてから先行回帰へ進む。本文読取期限はこのAxum sampleのContent-Type欠落だけに適用し、正常JSONと元client testは維持する。生成世代のmanifest/binからnative unitを実行し、0件やignoreを成功と数えない。修正後CI成功と#80のmain反映は未確認で、Phase 4のQ002も未承認。

### 承認Aの修正後ローカル確認

ADR→実行配線→tests-only→sample実装の順にcommitした。独立レビューのP2検査穴をhandler直接回帰で補い、元HTTP caseを維持して分割送信・4097byte正常JSONの413を追加した。High/保存Low各native8、HTTP19、不正port3が成功した。Python helper5/artifact9/CI52、site90も成功。生成applicationのstrict clippyは元generated main.rsのneedless_return2件で失敗し、allow・生成patchで隠していない。本体strict clippy成功とは分ける。

修正後4 OS CIは未確認。#80のbaseを#79のfeature branchからmainへ変更して、修正headのCIを確認する。#79の成功と、この修正の成功を混同しない。Pool/Txの内部は[既存Rust再利用比較](sqlite-pool-rust-reuse.md)を具体案へ反映し、自作pool/driverに確定していない。Phase 4のpublic API・hooks/依存・cleanup/close policyは未承認で、実装は開始しない。

## Phase 3 main反映とPhase 4の承認

2026-10-06。#80 head `35038940`のchecks run `37387962329`とwebsite run `37387961643`はattempt 1で成功した。4 OS・VSIX・IntelliJ IDEA・PyCharm・merge gateの成功を確認し、Windowsの実ログでもAxum High／保存Lowの成功を確認した。その後ユーザーがmain `f10cb64`へマージし、tree `e45dbede`の一致を読み戻した。エージェントはmerge・版更新・releaseを実行していない。過去のWindows失敗記録は残す。

同日、ユーザーがQ002の選択1を承認した。公開API・SQL制限・cleanup／close policyとruntime rusqlite hooksを[ADR 010](adr/010-sqlite-transaction-boundary.md)へ固定した。追加wrapperの依存承認は含まれない。標準APIを先に受理して未完成runtimeへ送らず、private一接続・一Txのfailing testsとsafe prototypeから開始する。Tx捕捉・nested shared・SQL opt-inの不足は設計とnegative corpusへ先に整理する。実装・検証結果は後続記録へ分ける。

## Phase 4: 一接続prototypeと次の判断

main `f10cb64`を基点に承認契約`3cce2f9`→tests-only `7accaec`→safe実装`3dff473`の順で進めた。追加wrapper、public registry、syntax、旧Db/APIは変更していない。private native22件、runtime151 unit＋5 doctest、全Rust92 suite・853件、fmt／all-target clippyがローカルで成功した。fuzz smokeは1000 mutation／95 checked Low-emit／16 nativeでpanic 0、CI helper52件成功。詳細は[結果](sqlite-session-results.md)。件数は包含関係にあり加算しない。

一接続の失敗探索で終端outcomeと自動rollback後commitのfake-success候補を修正した。禁止PRAGMAのstep拒否でAFTER trigger前のINSERTが残るnative挙動も再現した。新testが未採用のstatement atomicityを要求していたため、REDと一次根拠を残し、deny action／先行効果／明示rollback／reuseへoracleを分けた。普通のErr後にnative activeなら継続可という採用契約、既存test期待や保証は変えていない。

15組の予定High／手書きLowを作り、parse-only 2件で30sourceの構文と元行anchorを確認した。semantic harnessは未配線で、新Txのcheck／Rust build成功と報告しない。[compiler境界](sqlite-compiler-boundary.md)にcanonical resource、実payload／capture facts、SQL所有化・opt-inと未指定capabilityを整理した。4 OS CIへnative session／parser入力の専用stepを追加したが、CI結果は公開後の確認待ち。

次の[判断案](sqlite-pool-adapter-decision.md)はgeneric deadpool Managerを候補にする。独立レビューでdetachを経由しない破棄とin-flight createをcloseが待つ必要を確認し、候補へ反映した。追加crate／featureと未指定capabilityはQ004へ残す。未承認依存を追加したり、private Driverをそのままpublic Poolにしたりしない。Phase 4全体とPhase 5は未完了で、merge・版更新・releaseはしていない。

### Q004承認とManager比較への継続

2026-10-06。ユーザーがdeadpool =0.13.1（managed／rt_tokio_1、default featuresなし）とdeadpool-runtime 0.3.1による比較試作、およびcapability表の初版値を承認した。[ADR 010](adr/010-sqlite-transaction-boundary.md)・invariants・設計書へ反映し、追加・更新が必要なら差分を示して判断へ戻す。既存Tokio／rusqliteと旧Db／High／Lowの契約を維持する。

一接続基盤はdraft [PR #81](https://github.com/disnana/Nagi/pull/81)、head `cfa65fa61de1f81d6acbebbfd4898541ba1ee5e1`で公開した。4 OS CIはこの記録時点では一部完了・全体確認待ち。次の比較はbranch `feat/sqlite-pool-adapter`へ分け、#81の成果を保持する。main `5fdfe49`のREADME code fence更新だけを取り込んだ。adapter試作はまだ未実行で、公開Pool／TxやPhase 4完了とは報告しない。

終了責任はManager::detachだけへ置かない。in-flight create取消、idle破棄、active返却、Object::take、最後のPool Dropの経路を含め、worker起動前の登録からnative close／joinまで同じownerで保持する。checkerの先行公開や独自pool algorithmへの置換は行わない。mainへのmerge・版更新・releaseは実行していない。

### #81の先行基盤CI成功

head `cfa65fa`のchecks run `37397252295`とwebsite run `37397251707`はattempt 1で成功した。4 OSの実ログでprivate session22件／parser2件を確認し、両JetBrains製品・VSIX・merge gateの成功も確認した。[結果](sqlite-session-results.md#pr-81の4-os-ci)へ記録した。#81はreview可能、未マージ。これはnative一接続coreの検証で、Q004のadapter比較や公開Pool／Tx、Phase 4全体のacceptanceはまだ未完了である。

### #81のmain反映とadapter比較中の反例

ユーザーが#81をmain `ff6f7d4c81c8cf49c2bca7abffb3083f681d5b9d`へマージした。tree `cb8c3d061110e9866c208004f54d1cedf9d9481d`は、成功headに先行mainのREADME code fence変更を取り込んだtreeと一致する。エージェントはmerge操作をしていない。

比較初版は共通native sessionを維持したまま、startup取消後のworker並存と、join通知がterminal cause公開に先行する反例を確認した。前者はdeadpoolの論理slotとnative終了、後者は完了通知と結果公開を同一視したことが原因。完了Stateの全履歴保持も公開runtimeには残さず、live recordと集約counterへ分ける。[比較方針](sqlite-pool-adapter-decision.md#論理slotとnative-workerの終了を分ける)を先に更新し、barrier回帰で確認する。公開Pool／Tx、multi-connection、captured Tx検査、sealed SQL、acquire期限への接続はまだ未完了。

### Q004の一接続比較・ローカル検証

generic deadpoolのManagerへ共通native sessionを接続した。stock permit・queue・recycleは再利用し、native close／joinをledgerで観測する。Object::takeのpermit先行返却でもworkerが並存する実反例を追加し、max_size=1限定のcreateはlive記録が空になるまで待つ単純な条件へ揃えた。cause公開後にcounterとlive記録を更新する。健康Objectの通常recycleは維持し、多接続へこの条件を流用しない。

最終source tree `58297cd1`で、native22＋adapter20＋比較1の43件、全92 suite・874件、runtime172 unit＋5 doctest、fmt／all-target clippyが成功した。既存fuzzは1000 mutation／95 checked Low emit／16 native・panic 0。別のSolが登録と結果公開の順序をレビューした。23組46の予定High／Lowはparser検査だけで、semantic harness未配線のまま。追加依存は承認済み2個だけで、既存版更新なし。

同native coreの単独debug測定は各4096 Tx、direct p50 106.370µs、deadpool p50 114.402µs。raw sample、再実行条件、REDとGREEN、保証の限界は[結果](sqlite-adapter-results.md)と[測定](../../benchmarks/results/sqlite-adapter-2026-10-06/README.md)に保存した。throughput、allocator count、Future size、本番性能は未測定。main `ff6f7d4`を取り込んだ後も実装treeは同一。

次のPRはmain向けに分離する。新adapterの4 OS CIは確認待ち。公開Pool／Tx、多接続・Options取得期限、capture検査、sealed SQL、Phase 4全体のacceptanceは未完了。Phase 5、版更新、releaseは開始していない。

### #82のadapter実装CI

main向け[PR #82](https://github.com/disnana/Nagi/pull/82)、head `a608f1a`のchecks `37402576311`／website `37402576023`がattempt 1で成功した。4 OSの実ログで43件のprivate試験とparser2件を確認し、Linux全検査、両JetBrains製品、merge gateも成功した。VSIX packageは変更対象外、releaseはskip。artifactの改行・hashと次の設計メモを修正した最終headでもCIを確認する。compiler／runtime／Cargo／CIのbytesは維持する。

一接続比較は成立したが、公開配線のacceptanceとは分ける。[次の縦切り](sqlite-public-slice-plan.md)には、sequential join観測の多接続での反例候補、stock待機からnative fenceへの予算、巨大capacityの確保を整理した。source reviewによる候補で、実行済みのP1として数えていない。任意上限・新期限・新依存を追加する必要が出れば判断案へ戻す。エージェントによるmain merge・版更新・releaseはしていない。

## 2026-10-06: main反映と設計・Docs整備

ユーザーが#81をマージし、main `ff6f7d4c81c8cf49c2bca7abffb3083f681d5b9d`を読み戻した。#81の[checks](https://github.com/disnana/Nagi/actions/runs/37397252295)と[website](https://github.com/disnana/Nagi/actions/runs/37397251707)は成功し、4 OSでnative22件・parser2件を確認した。上の「CI待ち」「未承認」は当時の作業記録で、現在の状態ではない。

Q004の依存とcapability表は作者が承認した。別branchの[PR #82](https://github.com/disnana/Nagi/pull/82)は最終head `5a1c676`で[checks](https://github.com/disnana/Nagi/actions/runs/37404345603)・[website](https://github.com/disnana/Nagi/actions/runs/37404345104)が成功。4 OSの実ログでnative22＋adapter20＋比較1、parser2を確認し、レビュー可能にした。mainへのマージは行っていない。private一接続比較を公開Pool/Tx、多接続、取得期限、capture検査の完成とは扱わない。

### 今回の監査と文書変更

添付の設計・Docs引継ぎは同じmainを基点にしていた。`AGENTS.md`、DESIGN日英、内部契約、ADR 006/008/010、checker/emit、Scopeとactorの実装、ownership/Option/Scope/actorの関連testsを照合した。公開版、main、#82、採用方針を区別する。compiler/runtime/依存/CI/公開意味論は変更していない。

[ADR 011](adr/011-language-behavior-and-docs.md)に18の方針ID、現行との差、根拠、不採用案、後続の移行・検証をまとめ、DESIGN日英・invariants・pipeline・ADR 006へ接続した。入門、ownership、error、syntax、async/concurrency、actor/Supervisor、Lowと各英語版を整備した。人間向けDocsとAI向け文書/skillは分離を保ち、AI文書の古い定数検査・世代生成の説明も修正した。

Sol 2人が値と失敗／並行処理とLowをまとめて担当し、rootが設計と実装境界を照合した。独立した読み取りレビューで、承認済みSQLite項目を未決へ戻す記述、未決のhandle消費規則を無条件の二重await拒否として固定する記述を修正した。非Copy結果の二重取得を防ぐ方向と、Copy結果も含む再awaitの細部は分けた。

### 実行した検査

Linuxで基点mainの`cargo build --locked -p nagic`が成功、`nagic --version`は0.1.10。文書だけの差分なので、Rust全suiteや4 OS全体を再実行した結果とは報告しない。

| 対象 | 観測結果 |
|---|---|
| 日英の掲載コードと、明示した補完main | 74ケースで`nagic check`→`build`→報告されたnative exe実行が成功し、期待stdoutと一致。数学module2箇所はimport元の完全例と一緒に検査 |
| 定義だけのactor断片 | 日英2件のcheck成功。単独のactor起動・実行成功とは数えない |
| 意図的な誤り | 日英のmove後使用2件、追加6件の計8件をcheck段階で拒否。代入move、shared非Copy field、nullable演算、unawaited呼出し、Future保存、非unit spawnを元の行と理由で確認 |
| 新しい4種類の完全例 | 代入/reinitialization、shared handle/payload copy、nullable、TEMPORARY Supervisor。CLI `run`でも出力一致 |
| 上記4例の保存Low | 独立コマンドのcheck/build/run成功、Highと出力一致 |
| 評価順の小例 | 引数/両operandの左→右とand/orの短絡を実行で確認。全式・全backendの証明ではない |
| 入力CLI | 日英で21の成功、abc/-1の非0終了を確認。大きな整数の2倍の範囲確認は例では省いていると日英に明記 |
| 既存supervised-service | `python scripts/verify_library_examples.py --compiler … --only supervised-service`成功。実HTTPで状態更新、業務409、JSON400、shutdown204、停止後503、SIGINTのgraceful終了を確認 |
| CIのDocs判定 | `python -m unittest discover -s scripts/ci -p 'test_*.py'`: 52成功 |
| website | 既存venvで`website/build.py --base-path / --out build/design-docs-site`: 90ページ、日英ページ対応・local links/anchors/assets成功 |
| Markdown・Python比較例 | 変更36文書の相対リンク721件・見出しの欠落0、Python比較例16個の構文確認成功。`git diff --check`成功 |

ローカル検査の明細、抽出したsource、各check/build/executeログは`/tmp/nagi-design-docs-verification/`に保存した。初回の追加負例検査は期待していた診断の単語が実際の文と違ったためwrapper assertが失敗した。Nagi側の拒否は成立しており、元診断を確認して比較文字列を訂正した。処理系や既存testsの期待値は変えていない。

`results.json`のSHA-256は`40d52379794dbe72a16fe015fdd84c2bf39db513b8aa4a9e069d0d1e96a8e1da`、保存Low/評価順の`followup.json`は`9ece2fd810adc5d2366d01dc101741dc2a2d13cc159435aff631510477ac5e19`。同ファイルはローカルの観測記録で、公開配布物ではない。公開後のPR CI結果はPR本文へ記録し、ローカル結果と分ける。

### 残る差・未確認

OWN-04の明示操作、ASYNC-03/04の結果handleと業務Errの分類、ACTOR-01のshared messageは後続実装。大枠を再質問せず、[Q-005〜007](open-questions.md#q-005-既存所有値の代入を明示する範囲)に未決を集約した。Copy表、構文/消費、故障型、検出時点、容量課金を今回勝手に決めていない。移行条件はADRへ記録した。

今回は人間によるブラウザー操作、全外部リンクのHTTP到達、Windows/macOSでの掲載例の再実行を行っていない。既存CIの4 OS成功を今回のコード例の4 OS確認と取り違えない。リリース・版更新・main mergeも行っていない。

## 2026-10-06: #82のmain反映と#83の競合解消

#82のmerge依頼に対し、確認時点ですでに2026-10-06 12:20 JSTにmainへ反映されていた。GitHubのmerge commit `7999bab40b0a85b23ddf13b230e9e2db2c7ac3c9`とorigin/mainを読み戻し、最終head `5a1c676`が祖先であることを確認した。エージェントによる二重mergeは行っていない。最新のreview submissions・inline threads・discussionは各0件。最終headのchecks `37404345603`／website `37404345104`は成功し、4 OS・両JetBrains・Ready to mergeも成功。VSIXとreleaseは対象外でskipだった。

#83へこのmainをmergeし、DESIGN日英・open questions・progress・adapter判断資料の5競合を解消した。#82の実装、43件の結果、46予定入力のparser限定、全履歴と測定は保持した。初回Docs監査のmain `ff6f7d4`は履歴として残し、現在の#82反映と分ける。ADR011の将来変更や公開Pool/Txが実装済みになったとは書かない。#83のmain差分は文書だけを維持する。

解消後にwebsite90ページ、変更36文書の相対リンク728件・見出し欠落0、Python比較例16個、CI判定52件、diff checkが成功した。74実行・8拒否の初回検証は掲載sourceのhash一致を再確認し、新しい4完全例は更新mainでCLI runを実際に再実行して出力一致。独立Solレビューでもcompiler/runtime/scripts/Cargo/CI/benchmarkがmainと同一bytesであることを確認した。更新後PR CIは公開後に確認する。#83のmerge、版更新、releaseは今回の承認対象ではなく、実行しない。

### #83の更新CIとユーザーによるmain反映

競合解消後の最終head `b8bf768`で[checks](https://github.com/disnana/Nagi/actions/runs/37414124387)／[website](https://github.com/disnana/Nagi/actions/runs/37414124147)が成功した。文書だけの差分としてRust/native/editor/releaseはskip、CI判定52件とmerge gateは成功。#83本文へ結果を反映した後、ユーザーが2026-10-06 13:36 JSTにマージした。merge commit `a3c947fbc587dabc0c9c0dfc39ff42f7f389251c`をorigin/mainから読み戻し、続くPhase 4 branchへ取り込んだ。エージェントは#83のmerge操作を行っていない。

## 2026-10-06: Phase 4のprivate多接続と独立終了観測

[設計](sqlite-multiconnection-design.md)とADR010参照を先に保存し、6件のtests-onlyを追加した。容量だけを接続し単一reaperを残した中間source `3a9824d`では、A active中にBのjoin publicationが到達せず、cleanup後のassertでexit 101を観測した。単なる未実装APIのcompile REDとは分けた。公開Poolの多接続バグを直したとは報告しない。

native live登録の容量確認とclosing/failedを同lockで確定し、observer先起動→closure内native起動/actual join→cause公開→completionにした。nativeのJoinHandleをasync createやchannelへ渡さない。slot選択・公平性・healthy recycleはstock deadpool、SQL/cleanupは既存session coreのまま。起動不成立をfake joinにせず別counterへ記録する。

Sol 2人で実装と独立source/fixtureレビューを分担した。レビューでnative起動失敗を7件目に追加し、未join DBの削除・二重panic、setup失敗時のcleanup、publication前のCの再pollも改善した。元43件の期待値を削除・緩和していない。public semantics、compiler、cfg(test)以外のruntime、依存、CI設定、版は変更していない。

ローカル実装source `711ed4b`でprivate50件、runtime179 unit＋5 doctest、全92 suite/881件、fmt/all-target clippyが成功。fuzzは1000 mutation/95 checked Low emit/16 bounded native、panic 0。CI判定Python52件、website90ページのlocal links/anchors/assetsも成功。新sourceの4 OS CIは公開後に確認する。原ログ・共通原因・分類・限界は[結果](sqlite-multiconnection-results.md)へ保存した。

単独のwarm直列・cap1概測は各4096 sample。direct p50/p95 65.629/138.899µs、adapter 113.731/159.469µs。条件、生データ、hash、再実行法は[測定](../../benchmarks/results/sqlite-independent-observer-2026-10-06/README.md)。本番性能、多接続throughput、cold起動、allocationやFutureサイズの比較ではない。

| 次の対象 | 現在の状態 |
|---|---|
| 取得期限 | stock logical待ちからnative登録まで同予算を保つ候補はsourceレビュー済み。0ms・取消・同task scopeのprivate先行テストは未実装 |
| 巨大capacity | 可表現性の下限検査だけでstock allocationを検証済みとしない。公開受理範囲は残る判断 |
| 公開Pool/Tx | Options/Failure/Parameters、registry/capture/sealed SQLの配線は未実装 |
| compiler契約 | 予定23組46入力はparser2件のみ。semantic/pass-fail/元位置/Rust buildの検証へ昇格が必要 |
| 次Phase・配布 | G-TX/G-POOLの公開acceptance、Phase 5、版更新、releaseは未着手 |

この縦切りはmain向けの別PRとし、今回のmerge承認を流用しない。取得期限・巨大capacity・公開配線を進める順序と条件は[後続計画](sqlite-public-slice-plan.md)を維持する。

## 2026-10-06: #84反映とprivate取得予算

ユーザーが#84を05:47 UTCにマージした。main `e7aff1d`を読み戻し、取得予算は`feat/sqlite-acquire-budget`へ分離した。#84の最終headでは4 OSのprivate50件とparser2件、Linux／両JetBrains／website／gateが成功している。[CI記録](sqlite-multiconnection-results.md#84のciとmain反映)を追記した。エージェントはmergeしていない。

設計・ADR・invariants→tests-only→compile RED→stock waitのみのruntime RED→native予算接続→GREENの順で保存した。独立Solレビューで、期限後の最初のpollを固定するoracle、task-local消費点まで進める隔離試験、setup失敗時cleanupを補強した。元50件を維持し、取得9件と比較1件を加えた。公開API・数値default・依存・CI設定は変更していない。

ローカルでprivate60、runtime189＋doctest5、全92 suite／891、fmt／clippy、fuzz1000 mutation／95 Low emit／16 bounded native／panic0、CI Python52、website90が成功。budget比較の未poll begin Futureは2040 byte、従来fixtureは2024 byte。p50/p95の大小は逆で、速度向上を主張しない。source provenance、原ログ、生sampleと再実行法は[取得予算の結果](sqlite-acquire-budget-results.md)へ保存した。新headの4 OS CIは公開後に確認する。

| 次の対象 | 状態 |
|---|---|
| native容量と独立join | #84 main反映、private4 OS成功 |
| 取得予算 | private先行9件とローカルGREEN、公開入口は未配線 |
| 巨大capacity | P2公開前ブロッカー。stock版の全slot確保をvalidation済みとしない。[判断案](sqlite-capacity-decision.md) |
| Options／Pool／Failure／Parameters | 公開runtime未実装 |
| registry／capture／sealed SQL | 46入力をsemantic検査／元位置／High・Low・Rustへ接続する作業が残る |
| G-TX／G-POOL・Phase 4 acceptance | 未完了。Phase 5・版更新・releaseは未実施 |

今回もmain向けの別PRにまとめ、merge承認は流用しない。未解決の公開条件は根拠・代替・検証条件を示して判断する。

## 2026-10-06: #84反映後の取得予算CIと、言語仕様の実装準備

ユーザーが#84をmainへ反映した。今回の監査基点は`e7aff1da0a36503d239d70cf5dbcf892655978e0`。続く[PR #85](https://github.com/disnana/Nagi/pull/85)はmain向けdraftで、private adapterのlogical待ちからnative登録まで同じ取得予算を保つ修正。公開Pool/Tx配線や新言語仕様ではない。

#85の最終head `908cefff7305205069f90dd3d6d2e194b055a191`で[checks](https://github.com/disnana/Nagi/actions/runs/37426163565)／[website](https://github.com/disnana/Nagi/actions/runs/37426163260)が成功した。4 OSの各jobでsqlite_prototype 60件、追加取得予算9件の名前、failed/ignored 0を原ログから確認した。予定入力の2 parserテストも各OSで成功したが、46入力のsemantic conformanceではない。Linux全suite、両JetBrains、必須gateは成功、VSIX/releaseは対象外でskip。review submissionsとinline threadsは確認時点で各0件。draftを維持し、merge/releaseは行っていない。

その後の依頼で、ADR011の明示move・spawn結果handle・子taskの業務Errと故障分離を、文書から段階実装まで広げた。Sol 2人がcompilerのCopy/consume/origin/checked planと、runtimeのScope/結果/取消/Supervisorを分担して監査し、rootの[実装計画](value-task-implementation-plan.md)を再レビューした。設計の大枠を未承認へ戻していない。具体構文・Copy表・handle故障型等の未決を採用済みともしていない。

新しいownership operationは入力の値とoriginを透過する必要があり、native関数呼出しの名前追加だけでは足りない。Copy表を変えながら全consumeで明示要求を行うと、引数・return・record等まで別の互換性変更になる。初版はcanonical標準operationを追加し、旧暗黙代入の受理を保ち、次の差分でnonCopyローカル単純代入だけを移行する案とした。結果handleはscopeの実join所有と業務値を分け、Supervisor terminal ErrのHTTP停止を維持する必要がある。

基点mainの既存7 suite・36件は成功。[移行前の9例](value-task-audit-results.md)はHigh/手書きLow/生成保存Lowでcheck 24回（受理18・期待する拒否6）、正常6例の18 native実行で出力が一致した。初回runnerのstdout/stderr観測誤りは原ログを残して修正し、全48 CLIコマンドを再実行した。入力・拒否期待・アプリ出力期待は変更していない。

DESIGN日英、ADR011、invariants、Q005/006を実装順へ接続し、入門ownership日英のCopy説明を現行と照合した。compiler/runtime・依存・CI設定・生成コード・test期待は今回の準備差分で変更していない。新しいmove/Taskは未実装。V1/V2の具体APIと移行対象を判断できる資料を作り、その判断までは公開契約の実装を止める。S1/S2の細部を今すぐ全て質問せず、後続に分ける。Phase 4の公開acceptance、Phase 5、merge/releaseの承認を流用しない。

準備差分10文書の相対リンク/anchor 319件、website 90ページのlocal links/anchors/assets、CI判定52件、diff checkが成功した。新しい意味論のtest期待を変えず、compiler/runtimeとCI入力も変更していない。文書PRはmain向けdraftに分離し、公開後のheadでCIを確認する。

## 2026-10-06: #85 main反映と#86設計監査の同期

ユーザーが#85をマージした。main `f7799fa46ed513432b76cdf08648fe0986d44d5c`を#86へmergeし、private取得予算の60件・測定・終了契約の成果と、言語移行前の基点main `e7aff1d`での36件・正常18実行の監査を両方保持した。上のdraft維持・未merge・move具体案判断待ち等は当時の記録であり、現在の状態ではない。#86はユーザーが非draftにした状態を維持し、エージェントはPR状態を変更していない。

その後のユーザー指示で、canonical `std.ownership.move`、Copy据置、所有する非Copyローカルそのものの通常代入だけを拒否する範囲は確定した。別作業branchで先行test・実装・検証を進めている。#86と公開mainのcompiler/runtimeにはこの言語変更を含めず、現在の暗黙代入を禁止とは書かない。task結果handle、故障型・未受取等の詳細は後続であり、moveを再承認待ちへ戻さない。今回のmerge解消で既存検査を再実行したとは数えない。

## 2026-10-06: #85/#86を明示move実装branchへ統合

#86の更新head `93ad01119cc9ee37e63197a408a0ee75e74f33f5`を、move実装branch `feat/explicit-move-contract`へmergeした。#85のprivate取得予算と60件の記録、基点mainの36件・正常18実行の監査を保持する。文書競合はこのbranchでのV1/V2実装済みという現在の記載を維持し、#86と公開mainにmoveが実装済みとは扱わない。#85はユーザーがmainへ反映済み、#86は非draftであり、過去のdraft・判断待ちの記録は履歴として残す。

moveはRust標準の`std::convert::identity`へ入力を値として一度渡す実装で、OWN-04の範囲とCopy表を保つ。実装は未マージ・未リリースであり、統合後の再検証は[実装結果](explicit-move-results.md)へ別に記録する。Taskは[設計案](task-result-handle-design.md)の段階で未実装。このmergeだけを新たなCargo/4 OS検証の成功とは数えない。

## 2026-10-06: moveの全回帰と#86 main反映

#86の競合解消head `93ad011`でchecks／websiteが成功した後、ユーザーがマージした。main `7d2d96a8bdba191e456f796bdf477b95bc34c57e`のtreeは検証headと同じ`890fe8a425ce93005c4478e39685d0d8f239ecfd`で、originから読み戻した。エージェントは#85/#86のmerge操作を行っていない。

moveの統合後head `29a4618`は全90 suite・899件、failed/ignored 0。以前の93 suite・902件はgraph_renderの子process再実行3件の重複を含んでいたため、原ログを保って訂正した。fmt／clippy全target、10プロジェクト19実行、日英7箇所の4完全例のcheck/runが成功した。移行前36件・正常18実行とは別に記録する。限定生成256、fuzz 10,000 mutation／128 native、Drop・temporary・by-value比較の有限検査と原ログは[実装結果](explicit-move-results.md)へ保存した。

V1/V2は独立したmain向けdraft PRとして4 OS CIを確認する。mergeとreleaseは別途確認する。S1/S2のTask契約は設計案の段階で、公開SQLite Pool/Transactionも未実装。取消要求をjoin完了、rollback要求を完了と扱わない。

## 2026-10-06: #87の仕上げ監査

#87の公開head `e408973`はdraft・競合なしで、必須CIとwebsiteが成功した。レビュー提出・inline thread・通常コメントは監査時点で各0件。Sol 2人が実装と公開move Docsを独立に読み直し、未解決のP0/P1を見つけなかった。レビューは任意プログラムの保証ではない。

local `44c38b7`で、Copy入力にmoveを使っても元が使える説明と、元を残すにはmove代入をcopyへ置き換える説明を日英で補った。既存9群の3-source native oracleへCopy元の再利用、裸の引数・return、match payloadの裸returnを追加した。compiler/runtime/依存/CIのbytesは公開headから変更していない。全90 suite・899件、fmt／clippy、7箇所の完全例check/run、website90ページが成功。件数は子processの重複を除く。

前回の確認2点はmoveの残件ではなく、S1の未受取handleと故障回復性だった。今回の委任に基づき、全Tの正常出口await/discardと、受取後も残るscope故障を次工程の初版方針に選んだ。過去の方針に必然的に含まれていたとは扱わず、別の設計branchに採用理由・ADRと実装前の検証条件を記録する。Task実装、全spawn移行、公開Pool/Txを#87へ混ぜない。

最新headのCI・draft解除条件・有限な検証範囲は[仕上げ監査](explicit-move-readiness.md)を参照する。merge・release・版更新は行わない。

## 2026-10-07: Task S1/S2完成・条件付き消費修正と0.1.11候補

その後のユーザー指示により、同じ文脈でTaskを次Nagi release公開まで進める承認がある。前節のmerge/release停止は当時の履歴。#87はmainへ反映済み。Task S1 #88はcompiler→公開runtime、ownership/正常出口義務、sticky fault、実join、110契約/三構文native/4 OS/独立review/測定/日英Docsを完成し、main `aee1987`へ反映した。agent運用 #89はmain `f65c6093`へ反映した。S2 #90は既存await/tryでmonitorの内側Errを親bodyへ接続し、HTTP旧spawnを維持した。専用7ケース×三構文・両公開例・全回帰・4 OS・独立review後、main `f9b25782`へ反映した。新fault昇格APIやSQLite公開化を追加していない。

release直前の独立Sol Maxが短絡RHS/lazy env fallbackだけのawait/discardでskip経路のTask義務を消すP2をnative再現した。#92はcheckerの既存branch合流で修正し、emitter/runtime/API/評価順/依存/版は維持。148契約、三構文7経路、workspace95 result block/936成功/failed0/費用ignored1、fuzz1000/panic0/bounded native16、4 OS、両IDE/Ready/website、独立post-fix未解決0を確認しmain `97e62f82b1f67dbcea699071477cbeec7388a713`へmergeした。headとmainのtree `3e8cac92b12394f0887f661df90b989679bc0cce`を読み戻した。

版PR #91の最終head `c2b227533b89268028ecb2d67e39492b7aa4e4a5`は4 OS・全回帰・配布・独立Sol Max未解決0・日英migrationを確認し、main `003a594de086383100016b7c75466da37705646c`へmergeした。source共通treeは`13b41a034bf463a9426c53327b42c4184a53c7ed`。版差分はworkspaceとlockの自package2件だけ、VSIX0.1.13は不変。main checks `37558874886`・website `37558874635`が成功し、既存workflowが[Nagi 0.1.11](https://github.com/disnana/Nagi/releases/tag/nagi-v0.1.11)を2026-10-07 02:11:53 UTCに正式公開した。

公開tag/source/latest、前版0.1.10とのnotes比較、全8assetsの実download/byte/SHA-256/GitHub digest、4archive release.jsonと同梱runtime版の一致を確認した。公開Linux archiveのcheckout外E2Eはexit0でTask三構文/12拒否・SQL/actor/local Rust/JSON/元位置も成功。公開mainの4 OS各148契約・native8・runtime17・public3・doc9・両例三構文・archive gate、Linux workspace95 result blocks/936成功/failed0/費用ignored1、fuzz1000/panic0/bounded native16を原ログで読戻した。IDE/VSIX等のmain skipは再実行へ数えない。既知のTask release blockerはない。次の測定・探索・内部整理と未採用公開API/SQLiteを分けた[最終引継ぎ](handoffs/2026-10-07-task-release-0.1.11.md)、[公開後artifact](../../benchmarks/results/task-release-0.1.11-published/README.md)が現在の正本。


## 2026-10-07: #94後のTask/spawn統合確認

ユーザーによる#94 merge後のmain `97b7242c2172c1d0c701a7b662294e4dbe268abf`を取得した。Task結果handleは#88 S1/#90 S2/#92として既にmain反映・0.1.11公開済みで、旧private bridge停止時点を現在の未実装状態とは扱わない。採用済み契約をsourceから照合し、#94の署名先行検査・Low括弧化・内部transport境界とTaskを組み合わせる追加3回帰を固定した。production/compiler/runtime/API/依存/版/CI定義を変更せず、Task/move公開済み状態の日英Docs/DESIGN/AI資料を揃えた。

今回148契約・三構文native・全回帰・例・fuzz・費用/保持・独立Sol Highと、今回PRの4 OSは[結果](task-spawn-post94-results.md)、[原ログ](../../benchmarks/results/task-spawn-post94-2026-10-07/README.md)、PR Checksに記録する。過去の公開CIと今回の実行を区別する。次担当は[最新引継ぎ](handoffs/2026-10-07-task-spawn-post94.md)から読戻す。この依頼ではmain向けPRまでで停止し、merge/release/version bumpは行わない。SQLite Pool/Transactionは別の次工程。

source/test凍結head `428bcb07f2700b31f712cc8d10e8bc0ec3d29ce0`の今回4 OS checks `37611344198`/website `37611343828`が成功。各frontend13（追加3含む）・Task148/native8/runtime17/public3/doc9・両例三構文・archive gateを原ログから確認し、両IDE/Readyも成功した。公開Docs/実行例/AI skillに残った導入対象表記の日英修正と原ログ追記はcompiler/runtime/test/依存/CIの66 hashが同一。追記後の最新HEAD CIをPR #95で別に確認して停止する。

## 2026-10-07: JetBrains GitHub Release接続

Task/spawn対応とは独立に、JetBrainsプラグインのIC/PC検証済みZIPを将来のmain版更新時にGitHub Releasesへ公開する経路を追加した。プラグイン0.1.0、Nagi/VS Code版、compiler/runtime/APIを維持し、merge・tag・公開は行っていない。release113件・CI suite59件・日英Docs・Sol High独立reviewを確認。実IDE検証と最新HEADのCIはPRで確認する。[配布契約と検証範囲](jetbrains-release-pipeline-results.md)を参照。

## 2026-10-08: SQLite public Pool/Tx APIのPR #99準備

main base 6765767からのSQLite public API source head e3e0ea3962bd847a9ffdaef4fdabb7598844459f（tree e6d986d13345743465b74dfeb34f005a66a4c924）をPR #99として作成した。PRはdraft、latest-head 4 OS checks/website確認前であり、merge・release・version bumpをしていない。SQLite新APIはNagi 0.1.11に含まれない。Q002/Q004のユーザー承認を維持し、実装は既存Tokio FIFO semaphore＋lazy adapterへ切り替え、deadpool/deadpool-runtimeを除去した。新crateやTokio/rusqlite版更新はない。public surfaceは8 type/resources、18 operationsで、既存db_* APIは変更しない。ALLOCATIONはfallible reservation失敗に限定し、universal OOM recovery保証を付けない。

runtime stage e78f35aではpublic oracle 71件、runtime library 219 passed / 1 ignored、external public API integration 1、Task対象3、doc-tests10、runtime clippy成功を記録した。socket sandboxの初回35 EPERM failuresはinfra failureとして残し、network-enabled rerunと混ぜない。compiler stage 0f9dd79 focused結果はlib127、resource registry4、SQL checker14、parser2、sqlite_public6の各pass。SQL alias/owned strのP2は独立reviewで発見され、0bebcd0でquery/all/execのSQL argumentだけmaterializationして後続Parameters moveを許す修正とTx-loan negativeを追加した。filtered native run3 tests green。最終semantic runnerは46/46 matched（14 accepted、32 checker rejected）、Task runner148/148、runner-oracleはSQLite4＋Task4 pass。7 positive caseはHigh/saved Low/handwritten Low三経路native、16 negativeはcheckerのcause/originを確認。High checkで拒否された負例はsaved Low生成・native executionがN/Aである理由を記録した。Case04はfunction pointerの受渡しで、callback本体を呼んだ証拠ではない。

PR #99 sourceのLinux workspace final testsはexit0。raw logの単純集計は95 result blocks/970 passed/0 failed/1 ignoredだが、subprocess重複を含みunique tests数ではない。全workspace fmtとclippyも成功。Docs code exact-matchとschema preflight、website 94 pagesのlocal links/anchors/assetsも成功した。schema preflightは通常schemaを使い、3 SQL literal checked、0 unsupported、Parameters bind uncheckedを3件出して成功。stage・command/cache・raw evidence hashと未確認範囲は[SQLite results](sqlite-public-results.md)、[handoff](handoffs/2026-10-08-sqlite-public.md)、[selected evidence](../../benchmarks/results/sqlite-public-2026-10-08/README.md)、[provenance](../../benchmarks/results/sqlite-public-2026-10-08/provenance.json)に記録する。

導入Docs PR #98は別branch/head bc6a76bでready、4 OS/Linux/IDE/package/gate/website CIを成功後に確認済みで未merge。checks 37701060949 / website 37701060552。4 OS raw jobsで各10 onboarding native casesがpassしたが、これはSQLite PR #99の検証ではない。詳細は[PR #98 evidence excerpt](../../benchmarks/results/sqlite-public-2026-10-08/logs/related-docs-pr98-evidence.md)。

追記の最終Linux source検証でrelease build、examples 10 projects/19 runs、no-SQL featureのCLI 7＋SQL 1 test、seeded fuzz 1,000 mutations/16 bounded native/77 checked Low emit mutations/panics 0、SQLite High＋saved Low native sample、local linux-x86_64 archiveのextracted verificationが成功した。runtime full suiteは219 passed/1 ignoredで、raw log中にSQLite 73 testsがある。runtime public-oracleの別snapshotは追加2件より前の71件。local archiveは公式配布ではない。

PR #99のe3e0ea3 initial checksは実行中で、latest-head 4 OS/websiteのreadbackは未確認。generated-versus-manual cost sampleはLinux x86_64で完了し、32回/条件のraw samples・per-sample allocation分布・共有host/キャッシュ条件を記録した。sampleは小さく性能差の有意性やthroughputを示さない。debug public-cost snapshot、distribution mock gates、source projection testを性能・配布済archive受入の証拠として扱わない。次担当はこのhandoffとresultsを起点にCIを追記し、PRのdraft/merge/release判断を別に行う。

2026-10-08 統合追記: PR #98のhead `bc6a76b`へPR #99をrebaseし、#98をbaseとする依存PRへ整理した。最終反映先はmain。双方の目次・CIを保持し、compiler/runtime/Cargo.lock・SQLite配布gate・費用harnessは検証済み`e3e0ea3`から差分なし。統合source `fbfebd3`でwebsite 98 pages、初アプリHigh/保存Low native 10件、SQLite日英コード一致、CI policy 59 testsを確認した。以降の公開headと必須CIの最終結果は[PR #99](https://github.com/disnana/Nagi/pull/99)の本文とChecksを正とし、この文書の作成時点のCI pendingを現在状態と読み替えない。
