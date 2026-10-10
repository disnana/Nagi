# JetBrains 0.1.3候補: 意味解析と標準Runの検証

2026-10-09 JST。[PR #104](https://github.com/disnana/Nagi/pull/104)はmain向けDraft。
baseは `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`。候補version 0.1.3、ID `com.disnana.nagi`。
公開0.1.2のZIP/更新経路は変更していない。merge、tag、正式release、Marketplace uploadは未実施。
ユーザーによるGUI試用は一部のHigh Run/標準Run表示まで確認済みで、全体の受入確認は未完了。範囲は統合検証の現在値に記録する。

## 2026-10-09 統合検証の現在値

production/test sourceは `3a044fb4e93a56fb3de93afbc7b8f05ad6306a1a`。後続のMarketplace案内commit `f4bc1620a18422d57394e5a4b3af9946e0388e26`はDocs-onlyである。さらにこの結果・handoff追記もDocs/evidenceのみで、Rust/Java sourceやtestsには触れていない。CI/Plugin Verifierの最終対象は統合後の最新PR HEADで再確認する。

| 検査 | 統合sourceでの結果/境界 |
|---|---|
| Linux generation-retention native | 48 tests成功。通常CLI/foreign-out exportの保持、native entry/lease guard、X journal再開、current-input/last-good競合、unknown/oversized recordなどを実プロセス・native bytesで検査。原ログは[run-retention/native-48.log](../../benchmarks/results/jetbrains-assistance-2026-10-09/run-retention/native-48.log) |
| JetBrains | IC EAP 2026.3 build `263.6259.32`、JDK 25/Java release 21で70 tests・0 failures/errors/skips、`buildPlugin`成功。matching compiler test binary SHA-256 `3f278636d81197acf35d320019975e438602e2ea2071d8e7082e965da0800744`。JUnit/XML、ZIP descriptor、command、offline環境は[readback](../../benchmarks/results/jetbrains-assistance-2026-10-09/run-retention/java-readback.json) |
| 全workspace | exit 0、raw 101 result blocks/1039 passed/0 failed/1既存ignored。subprocess再実行を含む集計でunique test数ではない。[workspace.log](../../benchmarks/results/jetbrains-assistance-2026-10-09/run-retention/workspace.log) |
| compiler lint/format | all-target compiler clippyとfmt check成功。[clippy log](../../benchmarks/results/jetbrains-assistance-2026-10-09/run-retention/compiler-clippy.log) |
| website・policy/release tests | website 102 pages、local links/anchors/assets成功。CI policy 63 testsとrelease 116 tests成功。詳細は[証拠README](../../benchmarks/results/jetbrains-assistance-2026-10-09/run-retention/README.md) |

Native実装は独立Sol High reviewで指摘された5件の保持/依存境界を修正した後に承認された。Windowsではnightly-only file identity APIを使わず、stableのnamespace参照とnative output bytes oracleを使う。Linuxの48件がWindows junction testを実行したことにはならず、そのWindows実行と全4 OS・4 IDE/Plugin Verifierは最新HEAD CI待ちである。過去`feafba8`のCIはこの統合HEADの成功へ移し替えない。

GUIではユーザー共有の画面でHighの `Point/f64 1.5 + 2.5` 実行、`Hello, Nagi!`/`4`、exit 0、上部Nagi Run構成および右クリックの標準Nagi Run項目を確認した。画面から実際に右クリック項目を押してこの実行を開始した因果、IDE build、導入compilerのexact source identityは確認できない。Low、補完、navigation、live diagnostics、Stop、trust拒否、project切替のGUI試用は未確認であり、CI fixtureだけで完了扱いにしない。

公開compiler 0.1.11は通常のCheck/Runに対応するがassist protocolを持たない。compiler 0.1.12候補は中止されており、0.1.3候補のsemantic assistanceには同じPR sourceの開発compilerが必要。0.2.0完成前の正式compiler releaseは行わない。GitHub `jetbrains-v0.1.2` Releaseの共通ZIPとchecksumはpublic API/asset URLで確認した。一方、Marketplaceのpublic update API/build feedは現在0.1.1のみを返し、ownerからの0.1.2 approval通知とは異なる。public endpointではowner review stateを判定できないため、その通知の撤回とは扱わない。公開DocsではMarketplaceの現行版を断定せず、listingのVersionsで対応IDE版を確認する案内にした。

RetentionはIDE-managed runに限定する。foreign-outへexportしたmanaged generationはimmutable CLI成果物同様に保護し、unknown metadata、link/reparse、failed staging、shared Cargo cacheを回収しない。通常CLI世代はimmutableで、任意Rust include/build script依存を全探索する契約ではない。世代数/総容量の厳密な上限、電源断耐久、普遍fsync、OOM回復保証はない。各OS filesystemや電源断での全条件試験も行っていない。

## 実装

compilerのchecker/ownership/source mappingを意味解析の正本とし、`assist --editor-input`と常駐`--serve`を追加した。
High/Low、native replacement、元ファイル/UTF-16位置、import/shadow/move/viewを既存checkerから取得する。
補完はその位置で有効なread候補、navigationはcompilerのexact reference target、diagnosticは通常High→Low→finalizeの最初のerror。
Java側はPSI reference/completion/annotatorのadapterで、独自type checkerは追加していない。

project単位のworkerと常駐process、350ms diagnostic/80ms completion debounce、bounded overlays/response、
immutable bufferとsource/config/compiler identity、cancel/timeout/reap、trust gate、source graph cacheを接続した。
未知dependencyの初回結果は破棄し、disk contentのpre/post照合でclosed import/nativeの未通知変更を検査する。
同path compiler更新は旧processを終了し再起動する。元位置を推測せず、unmapped errorはfile-level messageにする。
Rust Windows canonical pathのverbatim drive/UNC prefixをIDE VFSと同じsource keyへ正規化した。

標準Run Configurationは`.nagi`/`.low`の右クリックRunから作成でき、上部Run/Stopを使える。
近くの`nagi.toml`ではmanifest entry/native設定を維持し、明示Run前にsourceを保存する。
未信頼projectを拒否し、launch直前にもtrustを確認する。Stopはprocess treeを終了し、外部副作用をrollbackする保証ではない。

## 初回意味解析のみのローカル観測（統合GC前の履歴）

以下は統合GC前の初回snapshotの値で、上の現在値と合算・置換しない。

| 検査 | 結果/保証範囲 |
|---|---|
| compiler assistance | 17 tests成功。overlay/常駐frame、scope/ownership/import、UTF-16、High・保存Low・手書きLowの通常frontend判定 |
| 全workspace | 99 result blocks、1012 passed/0 failed/1既存Task cost ignored。ログ集計はunique tests数とは限らない |
| fmt/workspace all-targets clippy | 成功 |
| bounded fuzz-smoke | text mutation1000/native16、panic0。coverage-guided fuzzではない |
| IDE Platform fixture | IC EAP 2026.3、JDK25/release21、69 tests/0 failures/0 errors/0 skipped |
| plugin archive | `buildPlugin`成功、common `nagi-jetbrains-0.1.3.zip`。CIで同じZIPを四SDKへ照合して昇格 |
| release/CI scripts | 116/63 tests成功 |
| website | 102 public pagesのlinks/anchors/assets成功。追加内部MDの最終checkは最新PR CIへ対応 |

原ログ・JUnit・source SHA-256は[保存記録](../../benchmarks/results/jetbrains-assistance-2026-10-09/README.md)。
shared warm Rust/Gradle cachesを用いた。clean buildとは呼ばない。socketはnetwork-enabledで実行した。
初回JDK21 cacheのjavac欠落、VFS fixture・owner通知競合、Gradleのtask option配置、
IDE bootstrapのreadonly home失敗は保存ログで区別し、失敗をskipへ変えていない。
bootstrapはXDG設定先をworkspaceへ指定して再確認した。

独立Sol High reviewでclosed dependency identity、idle compiler replacement、自動retry sequencingのP2計3件を修正した。
`e2538b9`→`48e256f`→`05460bc`のreviewとRED/GREENを契約記録へ保存した。
最後のretry fixtureは明示要求一回のみで自動再解析を確認し、reviewerもsource/4 targeted tests/buildPluginを再確認した。
Windows source key追加差分の独立確認と、最終HEAD CIの結論はPR本文へ追記して読戻す。

全workspace CIで既存SQLite Busy fixtureの0ms acquire raceを観測した。
rollback完了replyとlogical checkout返却は別なので、既存observer `wait_returned(2)`で返却を確認してからidle取得する。
Busy assertions・取得予算・runtime/Public SQL APIは変更しない。独立reviewとtargeted1件でbarrierを確認した。

## 性能/制約

optimized常駐protocolの7-run、10/100/500 localsのp50約166/182/318ms、最大186/188/363ms。
同時Gradle/build中の共有Linux環境で、IDE debounce/PSI/描画を含まない。
Platform fixtureでcache 500 lookupはmean約0.133ms/追加launch0。GUI end-to-end SLAは試用後に決める。
source graphは毎request再checkし、incremental checkerではない。

record field declaration、`stdlib:`仮想sourceへのjump、entry graph外、未保存manifest、新規physical未保存file、
複数syntax errorの回復、VS Code providerのassist移行は未対応。completion read候補は任意consume/call引数を承認しない。
READY後の外部変更通知はIDE VFSへ依存する。disk照合は外部writerとatomicなfilesystem transactionではない。
保存上限と合計8 MB disk snapshotにはmemory/読取り費用がある。

## 配布/GUI gate

最新PR HEADのNagi checks/website、四native platform、IDEA/PyCharm stable/EAP各test/Plugin Verifierを必須とする。
過去headの成功やskipを最終headへ移し替えない。Actionsの`release-jetbrains`と`release-windows-x86_64`を使う。
公開compiler0.1.11はassist非対応なので、同じPR HEADから作った`nagic.exe`を使い、ZIP内`release.json.commit`とchecksumを照合する。
[日英GUIチェックリスト](jetbrains-semantic-assistance-gui-checklist.md)を両IDEで実施し、GUI承認前にmerge/releaseしない。
公開compilerとの同梱/正式公開時期もrelease前に揃える。

Security Foundationは別PRへ進める。最新mainでは#100/#101はユーザーによりマージ済みで、SF01を再実装しない。
採用済み依存順SF05（literal Query/Parameters、旧Db/dynamic SQLの移行）→SF02（永続Session）→SF03（CSRF/CORS）。
このJetBrains PRへSecurity FoundationのAPI変更は混在させない。
