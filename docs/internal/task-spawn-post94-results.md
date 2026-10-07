# Task/spawn: #94統合後の確認

更新: 2026-10-07。対象mainは`97b7242c2172c1d0c701a7b662294e4dbe268abf`（[#94](https://github.com/disnana/Nagi/pull/94)のmerge）、treeは`ef8873dcd5fad802f86f9fd6fc0eb85aa5ec2801`。

## 現在の完成状態

Task結果handleは[#88](https://github.com/disnana/Nagi/pull/88)、Supervisor monitorの既存APIによる移行は[#90](https://github.com/disnana/Nagi/pull/90)、条件付き消費の義務合流修正は[#92](https://github.com/disnana/Nagi/pull/92)としてmain反映済みで、[Nagi 0.1.11](https://github.com/disnana/Nagi/releases/tag/nagi-v0.1.11)に公開済み。旧private bridge時点の引継ぎを現在の未実装状態と扱わない。公開tagのtargetは`003a594de086383100016b7c75466da37705646c`、正式公開日は2026-10-07。今回、その実装を重複追加せず、#94のcompiler修正との統合を確認した。

[ADR 012](adr/012-task-result-handles.md)、[language-invariants](language-invariants.md)、[S1結果](task-handles-s1-results.md)、[S2結果](task-handles-s2-results.md)、[条件付き消費修正](task-conditional-consumption-fix.md)が採用契約と過去の検証根拠。Taskはscope内で結果を一回受け取る非Copy/non-Clone/non-sharedのhandleで、全Tに正常出口時のawait/discard義務がある。moveは義務も移す。escape拒否・TaskFailureのkind/message・sealed生成は接続済み。

業務Resultは受取Resultの内側に残り、そのErrで兄弟をcancelしない。panic/異常取消等はsticky scope faultであり、受取・matchで解消しない。fault観測、兄弟cancel要求、全actual join、scope出口Errを区別する。body元Errとlegacy Errorのkind/messageを保持する。discardは受取放棄で、detach/停止/close/join省略ではない。同期parent Dropはabort要求まで。旧statement spawn専用scopeとTask混在scope、Supervisor/HTTPの既存故障連携を維持する。

## 今回の差分と追加した契約

productionのcompiler/runtime、公開API、依存、CI定義、版はbaseと同一。変更は`frontend_contracts`の追加3群、Task/moveの公開済み状態を揃える日英DESIGN・公開Task Docs・AI資料、および今回の結果・引継ぎ・原ログ。

| 追加回帰 | 固定した観測 |
|---|---|
| `task_results_preserve_flat_arithmetic_and_right_grouping_in_three_sources` | 129項の平坦な左結合式を子の業務Result::Errで返す。右側括弧の`20 - (5 - 2)`も残す。High、元Highを削除した保存Low、手書きLowがcheck後native実行し、`-112`とscope正常出口の`7`を出力する |
| `forward_invalid_task_result_signature_is_rejected_at_the_declaration` | 後方定義の不正`Result[i64]`をspawn/await/nested try経路から参照する。三構文ともparse成功後、checkerがpanicせず型引数数の診断を宣言の元行へ返す。CLIも同診断・元位置でexit 1 |
| `task_results_from_imports_cross_the_internal_low_transport_boundary` | 各source <2 MB、import合計 <8 MBのasync文字列返却2件をTaskとして受取。統合Low >2 MBのHigh経路はcheck/native成功し、`2000000`と`7`を出力する。同artifactを保存Lowとして新規入力すると2 MB source上限で拒否される |

#94は不正署名の先行検査、演算子優先順位に応じたLow括弧化、内部Low transportの別予算とCLI workerを修正済み。今回の3群はその上のTask統合回帰であり、新たな不具合のRED→GREENとは報告しない。sourceサイズ・import合計・深さ・work・内部transportの各防御を変更していない。保存Lowも新規入力の予算に従うため、上限超過の統合artifactを再入力して成功する保証を足さない。

## 検証と測定

今回のコマンド、原ログ、測定用生成Rust/手書きRust、source/artifact SHA-256は[artifact](../../benchmarks/results/task-spawn-post94-2026-10-07/README.md)に保存する。Linuxの既存Cargo/native cacheを再利用し、debug symbolsなし・incrementalなし・jobs 2で実行した。socketを使う回帰にはnetwork権限を付けた。clean buildではない。

Task入力登録は148入力/74 High-Low対、契約runnerは148/148一致、runner自身の段階・診断・元行oracleは3群成功。追加後のfrontend契約は13群成功。公開library例は15 check/run、アプリ例は10project/19実行が成功。固定seedのfuzzは1000 mutation、parse拒否731、check拒否192、checked Low emit 77、panic 0、bounded native 16。有限の入力・scheduler探索である。

生成/手書きRustの同一TaskScope保証・current-thread Tokio・releaseで、128/1024/8192件の受取/discardを25 loop×7反復した。6条件でcalling-thread allocation数・byte・reallocation/deallocationが一致し、batch Futureは双方受取416 B/discard408 B。8192件は受取32,773 allocation/3,342,740 B、discard32,809/5,538,236 B。生成binaryは1,258,752 B、手書きは1,234,320 B。時刻生値は保存したが、共有hostで全回帰・buildと同時進行したため速度優劣や同条件のcompile時間を主張しない。bare Tokioとの保証の違う比較ではない。

別の`cfg(test)` runtime費用oracleは明示`--ignored`で1群を実行した。1024×64 B payloadは完了未joinとjoin済み未受取の両状態で65,536 Bを保持し、handle Dropでpayloadが0になる。scope operation後にentry 0となるがHashMap等のcapacityは残る。primaryと127 related faultの保持も観測した。cfg(test)の内部layout/測定workloadはpublic生成batchと別であり、Futureサイズ・allocationを直接比較しない。通常全suiteで費用用1 ignoredを成功実行に数えない。

fmt/all-targets clippyと全workspace回帰はexit 0。原ログは95 result blocks/944成功（image_child子processの3再実行を含む）、重複を除くと92 blocks/941成功、failed 0/費用用ignored 1。Docs/siteは92ページの全local links/anchors/assetsに欠落0。今回PRの4 OS CIはPR ChecksとPR説明の読戻しを参照する。過去の0.1.11/main CIを今回branchの実行と数えず、skip/filtered/infra失敗も成功実行へ足さない。

Sol 6.1 Highの独立read-only reviewはTask公開metadata、checker義務、sealed生成、runtime lifecycle、旧spawn/S2、#94と追加3群を照合した。`ai/language.md`の追加std module公開状態のP3表現を修正し、再読戻し後の未解決指摘は0。reviewerは今回のテストを独立再実行していない。成功報告自体をruntime実証の代わりにしない。

## 未確認範囲と次工程

同期Dropで全join完了、non-yielding処理の強制停止、外部副作用rollback、任意Rust Drop/panic payloadの普遍回復は保証しない。multi-thread全allocation、他targetの費用、長時間scheduler網羅、clean build比較は未測定。#94で明記した極端に長い平坦ASTのstack上限も残す。

採用済みS1/S2の残実装はない。次の候補は、費用/保持の実運用条件追加、生成探索と縮小corpus、根拠のある内部整理。個別Task cancel/close/detach、scope外handle、一般Future保存、公開related cause/recoveryは未採用の新仕様であり、完了のために自動追加しない。[SQLite capacity/Pool/Tx](sqlite-capacity-decision.md)は別工程。このPRではmerge、release、version bumpを行わず、最新source headの必須CIとPR読戻しまでで停止する。
