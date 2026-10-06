# Sol 6.1への引継ぎ: Task結果handleのStage 1

更新日: 2026-10-06。**停止地点は、先行RED・private Task bridge・文書・CIの検証まで。Taskのcompiler/public runtime接続は開始していない。** ユーザーの最新指示で、ここを自然な区切りとして停止し、次の大きい実装へ進まない。

この文書はNagiそのものの開発向け。Nagiでアプリを書く資料は`ai/README.md`。最初にrootの`AGENTS.md`を読む。以下はrepository内のsource・保存ログ・GitHubの読戻しを根拠にする。AIの成功報告や会話の記憶を実装済みの証拠にしない。

## 1. main・branch・PR

| 対象 | 確認した状態 |
|---|---|
| main | `9ba4a104be6f65dba61eda0c7b1ef0f98c892cf7`。ユーザーによる#87のマージ |
| #87最終head | `5985e1b0c9f063f0a7153b3a79e8847f53765035`。このhead以後に変更していない |
| #87/main共通tree | `6986c81b76649b9ab8f6bf4867270fea51ff35cd`。明示moveの最終sourceを維持 |
| 今回の公開branch | `feat/task-result-bridge-stage1` |
| 今回のPR | [#88](https://github.com/disnana/Nagi/pull/88)、**main向け・draft・未マージ** |
| Stage 1検証head | `6223ad222016962328f9b0c2bedd0b90e36b38e5`。後続の引継ぎ/結果追記は文書・artifactだけ |
| 検証CI | [Nagi checks 37467579939](https://github.com/disnana/Nagi/actions/runs/37467579939)、[website 37467579497](https://github.com/disnana/Nagi/actions/runs/37467579497) |

引継ぎ自体のcommit SHAを本文へ自己参照させない。次のセッションではPR #88の実際のheadと取得したbranchのtreeを読み戻す。ローカルcommitとGitHub APIで作成したcommitはSHAが異なる場合があるため、同じsourceだと判断する際はtree・`provenance.json`のsource SHA-256を比較する。ユーザーがmainを進めていたら、上の固定headを最新mainとして扱わない。

merge、release、version更新は今回行っていない。次の担当にも自動実行の承認はない。

## 2. 完成した範囲と、まだないもの

| 項目 | 状態 |
|---|---|
| 明示move V1/V2 | #87としてmainへ反映済み。意味論・Docs・回帰は既存資料にある |
| Task初版の意味論 | ADR 012として採用済み。再承認待ちではない |
| private Task bridge | `runtime/src/task_bridge_prototype.rs`に実装。`runtime/src/lib.rs`の`cfg(test)`だけで接続。公開APIではない |
| native bridge検証 | 最終16群成功。Tokioの実join・取消・Dropを観測 |
| 先行High/Low入力 | 28組・56入力を登録。50の新Task契約は未達のまま |
| future contract runner | parse/name resolution/checkを区別。runner自身のoracle 3群が成功 |
| `task = spawn ...`構文・Task型 | **未実装**。通常のNagiからは使えない |
| Task ownership・scope義務・escape拒否 | **未実装**。Rust private handleのconsumeだけでは代用しない |
| TaskのLow/Rust生成・public TaskFailure | **未実装**。High→保存Low→nativeのTask conformanceも未実施 |
| Task allocation・速度・Future size | **未測定** |
| 旧spawn/Supervisor/HTTP移行 | 変更していない。機械置換禁止 |
| 公開SQLite Pool/Transaction | 今回着手していない。capacity課題も未解決 |

この段階では「実装途中」はprivate bridgeから公開言語へつなぐS1全体であり、compilerの部分的なTask実装が隠れているわけではない。`compiler/src/`、旧`runtime/src/concurrent.rs`、Cargo依存・版には今回のTask接続を入れていない。

## 3. 検証の正確な現在地

| 検査 | 観測した結果 | 保証していないこと |
|---|---|---|
| `cargo test -p nagi-runtime task_bridge_prototype::tests::` | private native 16群成功 | NagiのTask受理、must-consume、scope escape拒否 |
| runtime lib全体 | network権限付き206成功 | 全言語の正しさ、任意RustのDrop/panic回復 |
| workspace `cargo test --locked` | 成功。93 result block・919成功のログ集計（子プロセスimage_child再実行3件を含む）、failed/ignored 0 | 新Task機能が完成していること |
| fmt・workspace all-targets clippy | 成功 | 意味論の証明 |
| Task入力登録検査 | 56入力・28High/Low対を確認 | parse/check/native成功 |
| runner oracle | 3群成功。段階・診断・元行不一致を誤成功化しない | 50の新Task契約 |
| 56入力の現compiler照合 | **6一致・50未達、exit 1** | 下記のparse REDを意味論の成功にしてはいけない |
| Markdown/site | 229 MD、local link 1928、90 HTMLページ、links/assets 9750。欠落0。外部URLは取得していない | コード例すべての実行保証 |
| source head `6223ad2`のCI・4 OS | checks/website/merge gate成功。4 OSでprivate各16成功、全package/Linux/IDE成功。引継ぎ追記headのCIはPR Checksで別に確認 | skipや過去headの成功を今回実行したことへ数えない |

6一致は、旧spawn/同名ユーザー関数のHigh/Low計4入力と、既存checkerの型不一致・元行のHigh/Low計2入力。**残る50入力は全てparser段階で落ちている。Task契約がcheckerで検証された成功は0である。** parser/import拒否、ICE、違う行の診断はnegative契約のGREENにならない。

`compiler/examples/task-contract-red.rs`のnegative期待はcheck段階＋diagnostic fragment＋primary行。既存checkerの型推論は実行するが、新Taskの型推論は未検証で、lowering・Rust build/runへは未配線。まだREDなのでCIの必須成功stepには接続していない。CIは入力登録・runner oracleとprivate bridgeを実行する。checker実装後には50件をGREENへ接続し、native conformanceを別に実行する。

## 4. RED→GREENの根拠と既知の失敗

[Stage 1結果](../task-bridge-stage1-results.md)、[保存artifact](../../../benchmarks/results/task-bridge-2026-10-06/README.md)、`provenance.json`にsnapshot・hash・原ログがある。

| 保存段階 | 失敗と修正 |
|---|---|
| 01 | bridge API未定義E0432のcompile RED |
| 02 | sender通知だけでreceiveが完了する弱い候補。cleanup gate中に返るnative RED→actual joinまで待つ |
| 03 | receiveでprimary faultを消す弱い候補。scope出口Okのnative RED→sticky faultを維持 |
| 04 | 上記修正後15群。元Error保持はまだ不足していた歴史的snapshot |
| 05/06 | getter未定義E0599、その後元Error消失のnative RED→最終07で構造化causeを保持 |
| 07 | 元Error kind/messageがprimary・related・clone・再join後も残る。最終16群 |
| runner-location-red | 相対fixture pathとSource loaderのcanonical path不一致で元行None。共通canonical化し、既存checker負例をHigh/Lowで確認 |
| compiler-final-red | 一度Lowコメントを`//`と誤記。parse失敗を保存し`#`へ修正。最終`*-corrected`は6一致・50未達 |

弱い候補の実行用分岐は最終runtime sourceに残していない。古いsnapshotを現行実装へコピーしない。

初回runtimeの35 socket PermissionDenied、workspace二度のdisk容量不足はinfra失敗として原ログを保存した。network権限、debug symbols省略、build jobs 2、native cacheの`/tmp`指定で再確認した。test期待やtest並列度は変えていない。最初のCIはrun値末尾`tests::`の未引用でYAML parseが失敗し、全コマンドの引用で直した。これらを契約GREENや4 OS成功に数えない。

## 5. High / Low / Rustとcheckの責務

- Highは読みやすい主構文。Lowは波括弧の別構文・生成内容確認・関数差し替えを当面の範囲とする。Low廃止・pointer/unsafe/C ABI等への拡張はこの作業ではしない。
- 現pipelineはHigh parse/名前解決/check→Lowテキスト→Low parse/最終check・封印→Rust生成→Cargo/rustc。Lowテキストを使う大規模内部rewriteは未採用。High・保存Low・手書きLowで型/ownershipの意味を揃える。
- #74以降のchecked factsとsealed planを維持する。emitterはcheckerの決定を実現する立場で、名前リストによるownership再推論・暗黙clone・勝手なstatic化はしない。
- Nagiがサポートする意味論内で、check受理後に生成RustがNagiで検出可能なtype/move/lifetime理由で拒否されるのはP1。parse成功、check成功、Rust build成功、実行成功を別に記録する。
- rustcへ委譲するのは手書きRust/crate API・trait/Send/Sync/Clone・link/target/依存環境など。委譲をcompiler生成ミスの言い訳にしない。
- 元Nagi位置へのdiagnostic mappingを維持する。対応のないRust spanを推測して元位置へ戻さない。Task runnerの元行修正はcompiler mapping本体の修正ではない。
- NagiはRust資産を使うbackend境界の言語。今回独自VM・scheduler・GC・HTTP/DB全面交換を追加しない。

詳しくは[compiler-pipeline](../compiler-pipeline.md)、[language-invariants](../language-invariants.md)、[DESIGN](../../../DESIGN.md)。

## 6. 変更禁止の採用意味論

### ownership / move

`move`は明示所有権転送、`view`は読み取り借用、`shared`は安全な共有、`copy`は独立複製。#87のmandatory moveは非Copy**所有ローカルそのものの通常代入**に限る。引数・return・field・matchへ不用意に広げない。Copy軽量値、既存の再帰Copy class/enum、新規生成値の自然なbindingを保つ。標準operationのcanonical identityを使い、未importのユーザー名`move`を占有しない。

移動後の元値利用、借用中move、shared親から非Copy field取得はNagiで拒否する。shared handleのmove、clone_sharedの所有者追加、payload copyは別操作。High/Lowの意味を同じにし、Rust生成に暗黙cloneを足さない。

### Task / fault / lifecycle（ADR 012採用済み）

- `task = spawn ...`で結果handleを得る。Task[T]はTによらず非Copy・非Clone・非shared。
- awaitはhandleを一回consume。正常scope出口では全TのTaskにawaitまたは明示discardを要求する。
- scope外、関数引数/return、field/container/wrapper、他taskへのescapeを禁止。moveはhandleと義務を一緒に移す。
- 業務Result::Errは内側の結果。兄弟をcancelせず、外側TaskFailureとflattenしない。
- panic/task failure/unexpected cancellationはsticky scope fault。受取・matchしても故障状態を消さない。
- fault記録→兄弟cancel要求→全task actual join→外側Err。最初に観測したfaultがprimary、後続faultは関連cause。観測順はscheduler順でspawn順ではない。
- body元Errは後続child faultで置換しない。既存locals cleanup anchorとscope bodyラベルを維持する。
- discardは受取放棄。detach、子停止、fault抑制、join省略、資源close完了ではない。
- cancel要求と終了確認、rollback要求と完了確認は別。同期parent Drop/unwindはabort要求までで、actual join完了を返せない。
- non-yielding処理の強制停止、外部副作用rollback、任意Rust Drop/panic payloadの普遍回復は保証しない。
- 旧statement spawnのunit/Result[unit,Error]・fail-on-ErrとSupervisor terminal→HTTP取消を維持する。新Taskの内側業務Resultへ機械置換しない。

## 7. 残る設計判断と技術リスク

意味論の大枠は決定済み。全T義務やsticky故障を再質問して作業を止めない。実装接続の推奨は[task-handle-implementation](../task-handle-implementation.md)に保存した。

| 項目 | 判断・注意 |
|---|---|
| TaskScopeの選択 | Task bindingを含む最寄りscopeだけ新bridge、旧spawn-onlyは旧Scope。nested scopeは独立。sealed planで選ぶ推奨、まだ未実装 |
| canonical標準type | SpawnBindが正式std.task metadataを取得し保存Lowへ残す。裸名Taskをbuiltin化しない |
| ScopeId / binding義務 | runtime ticketと分離。loop再checkで安定、同じ深さ/同じ行の別scopeも区別。単なるmovedフラグだけではmust-consume不足 |
| discard API | unit返却の推奨。privateのResult返却をそのまま標準APIにするとResult裸式拒否に衝突する |
| TaskFailure API | opaque value、kind/message読み取り。Kindは4値Copy enumの既存resource constant経路、messageはFailure-origin view。具体名は接続案、公開API未実装 |
| scope出口 | legacy primaryの元Error kind/messageを維持。TaskFailureの暗黙Error変換は足さず、scope出口Errorと受取TaskFailureを分ける |
| public bridge | private16oracleを本体へ移す案。private/public意味論の二重実装を残さない |
| retire / 保持量 | privateのrecord毎全entry retainはO(n²)候補。対象ticket退役＋drain終端sweepを検証。勝手なcapacity/timeout/observer/独自GC追加はしない |
| allocation / Future | oneshot、receipt Arc、HashMap、cause Arc/message、関連faultを保持。Task payload Clone不要≠allocationなし。長いbodyの完了未join/未受取Tも測る |
| 公開SQLite | capacity/allocation境界と公開Pool/Txは別工程。今回のTask変更へ混ぜない |

既存契約から一意に決められないownership/cancellation/fault変更、syntax/APIの大幅変更、意図的互換性破壊、Low廃止、backend/runtime全面交換、範囲外SQLite拡張はStop。候補・推奨・理由・移行/検証案を作ってユーザーへ確認する。S2の明示service fault昇格の具体APIと公開Pool/Tx capacityは、今回承認した実装と扱わない。

## 8. 最初に読むファイル

1. `AGENTS.md`、この引継ぎ、`docs/internal/task-bridge-stage1-results.md`。
2. `DESIGN.md` / `DESIGN.en.md`、`language-invariants.md`の現行表とS1予定表。
3. `adr/011-language-behavior-and-docs.md`、`adr/012-task-result-handles.md`、`task-handle-implementation.md`、`task-result-handle-design.md`、`task-result-handle-review.md`、`value-task-implementation-plan.md`。
4. `runtime/src/task_bridge_prototype.rs`、`runtime/src/concurrent.rs`、`runtime/src/lib.rs`。
5. `compiler/src/ast.rs`、`parser.rs`、`check.rs`、`check/checked.rs`、`stdlib.rs`、`modules.rs`、`source.rs`、`emit.rs`。このrepositoryに`lower.rs`/`resource_registry.rs`を仮定しない。
6. `tests/task-handles/contracts.json`と入力、`compiler/examples/task-contract-red.rs`、`scripts/verify_task_handle_contract_inputs.py`、`.github/workflows/ci.yml`。
7. `compiler/tests/scope_runtime_contract.rs`、`scoped_tasks.rs`、`explicit_moves.rs`、`conformance.rs`、resource registry/capabilityの独立inventory。

進捗正本は[progress](../progress.md)、未決事項は[open-questions Q-006](../open-questions.md#q-006-spawn結果handleと業務errtask故障)。公開Poolの課題は[sqlite-capacity-decision](../sqlite-capacity-decision.md)。PR [#87](https://github.com/disnana/Nagi/pull/87)・[#88](https://github.com/disnana/Nagi/pull/88)も実際のhead/CIを読み戻す。

## 9. 次に行う作業（再開の指示を受けてから）

1. 現main/PR #88/head/tree、変更中のfile、最新CI/未解決reviewを確認。Stage 1 source hashesを照合し、今回停止後の他人の変更を上書きしない。
2. 接続判断をADR/contractへ同期してから追加REDを保存。裸`move(task)`で放棄、両分岐消費、消費後再生成の合流、nested旧scopeから戻って外Task受取、同深さ別scope、Failure.message借用中move、引数/wrapper/他task escapeを優先する。
3. 専用SpawnBind、canonical std.task metadata、私有ScopeId/binding義務を追加。shadow/再代入/branch/backedgeで義務を消せないこと、await/discardの二重consume、move後元値利用をcheckerで拒否する。
4. public runtime bridge接続とsealed Spawn/Await/Discard/Scope planを揃える。旧scopeを全面置換せず、元cleanup/エラー変換と評価順を維持する。
5. 保存Low/手書きLow、High→Low→nativeで正負・元位置・TaskFailure sticky・業務Err兄弟継続・全actual joinを実証する。compile-only/stubをruntime実証にしない。
6. 旧spawn利用を分類して互換を検証。Supervisor/HTTPは無変更から始め、S2は別の具体設計・専用oracleで進める。
7. 全回帰・fuzz/生成探索・4 OS、コスト測定、日英Docs、Sol独立レビュー。縮小反例を恒久corpusへ入れる。
8. main向けdraft PRを更新し、最新source headの必須CIを確認して報告。merge/release/版更新は別途指示までしない。

Sol 6.1は同領域をまとめて担当する。例えばcompiler一式とruntime一式の少人数分担にし、細かいagent分割を増やさない。独立レビューを担当の成功報告で代用しない。高速モードを明示する利用はLuna Maxだけというユーザー方針がある。

## 10. 重要なコマンドと保証範囲

通常のtoolchainで次を実行する。必要なsocket/依存cacheを用意し、infra失敗は隠さない。

```sh
python scripts/verify_task_handle_contract_inputs.py
cargo test --locked -p nagic --example task-contract-red
cargo run --locked -p nagic --example task-contract-red -- --report /tmp/task-contracts.json
cargo test --locked -p nagi-runtime task_bridge_prototype::tests::
cargo test --locked -p nagic --test scope_runtime_contract --test scoped_tasks --test explicit_moves --test conformance
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked -p nagic --example fuzz-smoke
```

三番目は現時点でexit 1が観測されている。コマンド成功を求めてexpectedをparse-failへ変えたり50件をskipしてはいけない。public bridgeへ移したら四番目のfilterとCIは新module名へ同時更新し、0件実行を成功として見逃さない。

今回のLinux環境は`/workspace/toolchains/{cargo,rustup}`、workspace Cargo cacheは`/tmp/nagi-container-flow-target`、native cacheは`/tmp/nagi-task-native-target`を使った。容量不足対策は`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`。環境固有pathをrepository既定へ固定しない。cacheを使った実行をclean buildと呼ばない。ログとsource hashは上記artifactに残る。

## 11. S1全完成のacceptance

- 50のTask先行契約と追加負例が正しい段階/元位置でGREEN。parse/import失敗ではない。
- Taskの全T一回consume/正常出口義務/escape禁止がHigh・保存Low・手書きLowで一致。
- checker決定をsealed planから生成し、暗黙clone/意味論再推論や新check/build mismatchがない。
- business Result Errで兄弟継続、faultを受け取ってもscope出口Err、cancel要求後に全actual join。元legacy Error・body元Errを保持。
- discard・未poll/Pending receive Drop・parent Dropの責任を分離。旧spawnとSupervisor/HTTPの故障連携を維持。
- private bridgeの16native oracleをpublic実装でも検証。fake native ID再利用・Drop場所・protocol誤用も残す。
- native全経路・全回帰・4 OS・fuzz/生成探索、Docs/DESIGN/ADR/contract、Solレビューを完了。
- 同条件の手書きRustと生成RustでFuture size、allocation/保持、時間・binary/compile costを測定。保証範囲の違うbare Tokio比較は別に明記。
- 未確認事項・制約を残してmain向けPRを完成。SQLiteやreleaseを混ぜない。

Stage 1の成功は、この全完成条件を満たした記録ではない。次の担当は上の差を埋めるところから再開する。


### このcloud環境での公開確認

CLI pushはGitHub書込み認証がなく失敗したため、接続GitHub APIでbranch/tree/commitを作成した。GitData ref更新だけではPR head/CIの同期が遅れた事例があり、正規のContents APIによる文書更新後にPRのheadとCIを読み戻した。refが進んだことだけでPRが検証済みと報告しない。PRを閉じて再開する方法は使わない。次の環境ではcredential readinessを別に確認し、この環境の認証状態を引き継がない。


## 停止状態の確認

引継ぎ対象のproduction sourceは4 OS検証head `6223ad2`から変更していない。追加したのは結果・原ログ・接続設計・この引継ぎのみ。Task checker/public runtime接続は未着手、50入力はparse RED、測定は未実施のまま停止する。次の担当はこの境界を保ったまま、再開指示を受けて最初のRED追加とchecker接続から進める。
