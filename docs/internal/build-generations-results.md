# Phase 2: build generation isolationの検証

2026-10-05。branch `fix/build-generation-isolation`、作業基点はPhase 1のPR #78 head `08199bf`。その後ユーザーが#78をmain `0107f37f0de6026533a4d67f52c6545054b71584`へマージし、headとのtree一致を確認した。Phase 2のPRはmain向けで、Phase 2の差分はmain未反映。以下は未リリースの開発branchにおけるローカル結果。Windowsを含む4 OS CIは未実行で、Phase 2のacceptanceはCI待ちである。

## 契約と変更

同じappの再buildがcache上の同じexeを上書きし、Cargoのtarget lock解放後のrunとも競合していた。[ADR 007](adr/007-build-generations.md)と承認済みQ-001に従い、canonical sourceと論理outから決まるpackage/app IDを維持し、世代固有のbinと成功artifactを分けた。依存cacheは共有する。

初回checkとinput保護の後、常設canonical out lockを取得し、取得後にも保護を再確認する。同じoutの生成、writing High check/lower/cost、Cargo、公開、互換projection、latest更新を調停する。非書込みLow check/lowerにはlock待機を追加しない。

世代ごとのLow/Rust/manifest、読み取り済みsource/provenance、Cargo.lockをstagingへ保存する。Cargo成功後にexeをコピーし、同じ親・深さの成功世代へrenameする。互換出力の更新後、閉じたtemp fileをrenameしてlatestを置き換える。旧成功世代は上書き・削除・killしない。runは選択済みの成功exe pathを保持し、起動前にlockを解放する。

## 先行回帰と実装中の発見

tests-onlyの初回・補強後の失敗は[進捗](progress.md#先行回帰の初回観測)に記録した。一定時間markerがないことだけで直列化を判定せず、先行Cargoのhold、OS `try_lock`の競合、後続writerの待機通知、解放後の終了・stdout・snapshotを観測する。孤児Cargoにも期限と完了statusを設けた。

| 分類 | 観測 | 対応 |
|---|---|---|
| P1 | stagingと公開先の深さが違い、公開manifestのruntime相対参照が壊れた | 同じ`generations`親内で公開し、公開後の参照先を検査 |
| P1 | projectionが旧latestへのhard linkなら、生成書込みがmetadataを破壊した | projectionとlatestを同じ出力集合として初回・lock取得後・finish時に保護。writing check/lower/costにも適用 |
| P1 | 旧CLIが受理する非UTF-8 cwdと長いsource basenameで、check成功後に新しいmetadata/bin生成がbuildを阻んだ | 未リリース差分で発見・修正。snapshotにOS pathの生unitsを保存し、bin名から長いstemを除く。package/app IDは維持 |

独立したfreezeレビューではowned 12ファイルのhash一致を確認し、新しいP0/P1は報告されなかった。その後Python readerとtestの2ファイルだけを更新した。metadataのtop-levelがobjectでない場合、またはschema versionがbool/floatの場合を明示的に拒否する。修正前は9 test中2 failure・3 error、修正後は9成功のログを保存した。Rust sourceはfreeze後に変更していない。

## ローカル検証

rootが各実行のexit 0を確認した。ログは実行workspaceの`/workspace/test-tools/compiler-rust-boundary-plan/`へ保存した。

| 対象 | 結果 |
|---|---|
| `cargo fmt --all -- --check` / `cargo clippy --locked --all-targets -- -D warnings` | 成功 |
| `cargo test --locked`最終再実行 | 90 suite、807成功、失敗・ignoreなし。`phase2-full-suite-after.log` |
| generation / CLI / output保護 / shared target / project | 23 / 7 / 10 / 2 / 1成功 |
| native diagnostics / dependency / installation | 30 / 9 / 3成功 |
| 拡大conformance | seed `305419896`・`3735928559`、各256生成case＋38固定corpus、各runのharness 5成功。登録確認は38 corpus・12 linked harness |
| mutation smoke | 10,000入力、parse拒否7,149、check拒否1,910、check後Low/生成941、panic 0。別に128 bounded native case。coverage-guided fuzzではない |
| SQL engineなしbuild・CLI/SQL | 8成功 |
| Node | VS Code 196＋HTML viewport 5、計201成功 |
| Python | native helper 9、CI判定51、release scripts 90成功 |
| application examples | 10 project、19 run成功 |
| website | 90ページ生成とリンク等の検証に成功 |

初回の全suiteでは`typed_errors`の`binary.parent == cache/release`という旧内部assertが1件失敗し、共有mutexのpoisonで後続7件も失敗した。Q-001に従い、成功metadataのnamespaceと対応するcache exe bytesの一致を検査するassertへ更新した。対象15件と全suite807件を再実行し成功した。stdoutや診断の期待を緩めた修正ではない。

Node初回のchild spawn `EPERM`は`phase2-editor-tests.log`に残した。同じテストを必要なpermissionで再実行し、`phase2-editor-tests-after.log`で201成功を確認した。metadata schemaの修正前後は`phase2-metadata-schema-before.log`・`phase2-metadata-schema-after.log`に残した。失敗を成功・ignoreへ置き換えていない。

## build測定

[全データとsummary](../../benchmarks/results/build-generations-2026-10-05/summary.json)を保存した。小さなstd-only appと空のlocal runtimeを使い、旧・新CLIで各15 build/runを3組、計90回観測した。生成Rustは45 pairすべてbyte一致し、入力とstdoutも各組で一致した。

warm中央値、単位ms。各欄は旧→新、各mode・CLIで7 sample。

| 組 | 同じ内容 | 内容変更あり |
|---|---|---|
| initial | 77.528 → 91.920 | 73.913 → 92.237 |
| repeat | 78.371 → 110.124 | 78.718 → 82.843 |
| alternating | 81.918 → 80.582 | 89.770 → 87.756 |

空Cargo targetの1 sampleずつはinitial 198.003→162.372ms、repeat 135.034→153.691ms、alternating 272.160→155.131ms。OS cacheまでcoldとは扱わない。binaryはこのfixtureで458,032→458,152 bytes、120 bytes増えた。

共有Linux hostの実行順・負荷によってばらつく。initial/repeatのwarm増加と交互測定の近い値の両方を残し、速度不変・高速化・因果的なoverhead上限は主張しない。runtime throughput、latency、allocation、RSS、Future frameは今回測定していない。以前観測したFuture frame +32 bytesは未解決である。

## 保証の限界・残課題

追加依存・新しいunsafeは導入していない。advisory lockは協調するNagi writerの境界で、sandboxではない。読み取り済み入力はsnapshotへ保存するが、外部Rust、path crate、runtimeの参照先全体はsnapshotしない。協調しない外部Cargo、悪意あるdirectory差し替え、OS crashへの耐久性、複数互換ファイル全体のatomic置換は保証しない。

失敗時は旧latestを維持して新世代をrunしない。互換出力は途中まで更新され得るため、その状態を診断し、全出力のrollbackを保証しない。Cargo.lockはTOMLとして同じ内容なら元bytesを保持し、解決内容が変わればCargoの更新を使う。cache/projectionと成功exeをhard linkで共有しない。

| 分類 | 残課題 | 次の行動 |
|---|---|---|
| acceptance待ち | Windowsの既存latest置換失敗・回復と4 OS/package/editor動作はこのhostでは未確認 | CI成功を確認してからPhase 2を完了し、Phase 3へ進む |
| P2 | raw非UTF-8 argvが`std::env::args`でpanicする既存問題。旧CLIでも再現 | cwd互換性の修正とは分けて追跡 |
| P3・設計のtradeoff | 成功世代と世代binが蓄積しdisk使用量が増える | 自動削除・killは追加しない。cleanup方針は別判断で、保持契約や公開policyと衝突する変更はStop |
| P3・既存の性能観測 | Future frame +32 bytesの原因・影響が未解決 | 別の同条件測定で調査。今回解消したと扱わない |

Phase 3のcharacterization/refactorは未実装。Phase 1のmain反映はユーザー操作であり、エージェントはmergeしていない。Phase 2のmainへのmerge、版更新、releaseは行っていない。
