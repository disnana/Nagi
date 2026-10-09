# JetBrains 0.1.3候補: 意味解析と標準Runの検証

2026-10-09 JST。[PR #104](https://github.com/disnana/Nagi/pull/104)はmain向けDraft。
baseは `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`。候補version 0.1.3、ID `com.disnana.nagi`。
公開0.1.2のZIP/更新経路は変更していない。merge、tag、正式release、Marketplace uploadは未実施。
Windows GUI試用はユーザーが実施する。

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

## ローカル観測

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
