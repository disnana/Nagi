# SQLite 公開 compiler 接続の独立レビュー

対象は immutable head `0f9dd793472c7e14a076288316852a8c3f861fc1`、base `676576724829e45b077b58628bfe2417e6cf3673`。加えて、レビューで発見したSQL alias拒否への親担当の未commit修正（check/sqlite.rs、sqlite_public.rs、内部boundary文書）を追跡評価した。開始時にAGENTS.mdを読み、固定headに対するcompiler/runtime/scripts/benchmark/exampleのworking差分がないことを確認した。public Docs/CIの進行中差分は評価対象外。source hashは `independent-compiler-review-snapshot.json`、主要固定sourceの元行は `compiler-review-oracles/*.fixed.txt` に保存した。

レビュー担当はコード・Docs・テスト・依存・期待値を編集していない。専用lazy adapter、deadpool除去、新crateなし、必要なAPI/Failure分類の変更はユーザー承認済みの設計として扱った。runtime `e78f35a`は既存の独立runtimeレビューを前提にし、本レビューではcompilerとのsignature/Passing/type/ownership接続を確認した。

**固定headには確認済みP2が1件あった。その限定修正と回帰をレビュー済みで、追跡差分を含むcompiler scopeには未解決の確認済みP0/P1/P2を見つけていない。** 最終headの全回帰、4 OS、完成した日英Docs/CI、immutable配布物のacceptanceは別途必要である。

## 確認した問題と追跡評価

### P2: SQL所有化後も一時的なSQL入力loanを保持し、有効なParameters moveを拒否する

確度: 高。固定head `compiler/src/check/sqlite.rs:110`、関連 `compiler/src/emit.rs:662`、`docs/internal/sqlite-compiler-boundary.md:29`。

成立条件は、owned strをSQLのviewとして渡し、同じcallの後続Parameters builderへそのstrをmoveすること。通常サイズの独立High/手書きLow例:

```nagi
return await sqlite.exec(tx, view(sql), sqlite.bind_text(params, sql))
```

固定headのcheckerは双方を元行3で `sql は同じ式で先に参照されています` と拒否した。証拠は `compiler-review-oracles/sql-alias.nagi.log` / `sql-alias.low.log`。SQLのsealed planはargument1をOwnedへ複製してからargument2のParametersを評価するため、この一時loanをcallの最後まで保持する理由はない。Q002と生成側のSQL所有化→後続move契約に反する受理範囲の縮小で、Rustへの漏れやruntime不正の再現ではない。

親担当の追跡修正はworking `check/sqlite.rs:110–116` でcanonical query/all/execのargument1だけhold_valueを省く。Txのreference loanや既にlocalへ保存したviewのloanは保持する。新 `sqlite_public.rs:529` はexec/query/allで同じstrをSQL view→bind_text moveとしてHigh/保存Low（High削除後）/手書きLowの3経路でnative実行し、insert数と返却文字列を検査する。:579の負例はTx借用中にTxをconsumeする別引数をcheckerで拒否する。

独立した小さい通常harnessで修正後ライブラリを用い、High/手書きLowのalias例のcheck→finalize→sealed SQL emission成功を確認した。保存local viewが残る負例は元行4で拒否、Tx消費の負例は元行5で拒否した。証拠は `compiler-review-oracles/post-fix.rs` / `.log`。親の `sql-alias-green.log` 本文は追加positive/Tx negative/既存SQL orderの3 tests成功を記録する。native三構文の追加再実行は親の観測であり、独立harnessはchecker/finalize/emissionまでである。

### P3: 最終native成功を指す内部文書が途中REDログを参照する

確度: 高。固定head `docs/internal/sqlite-compiler-boundary.md:49` は最終統合ログを `compiler-public-native-matrix.log` とするが、同原ログはhandwritten Lowのextern改行誤りで5成功/1失敗と終了している。最新 `compiler-final-focused.log` の6成功は本文から確認できた。親の文書修正は最終成功をfocused logへ向け、古いログは途中REDと明記している。この追跡修正を確認済みで、コードblockerではない。

## 主要境界の評価

| 対象 | 根拠 | 結論・成立条件 |
| --- | --- | --- |
| canonical registry / inventory | stdlib.rs、stdlib/sqlite.rs:4/:179/:275、resource_contract_characterization.rs:316/:468/:622/:674 | 新8resources/18opsを独立手書き期待と完全集合で照合する。既存resource/operation期待を削除していない。native path、signature、Passing、constants、Failure field型を公開runtimeと照合した。ユーザーの同名Pool/Tx等をresourceの綴りだけで認識しない。 |
| 用途別payload遍歴 | capabilities.rs:7/:99/:112/:123、check.rs:1936/:1968/:1999/:4211/:4253 | function pointer署名は実payloadに含めず、native inline/shared役割とclass/enum fieldを追う。共有化は非shared SQLite資源をnestedで拒否する。copyはsharedで停止してArc handle Cloneを許す。SameTaskはcanonical Txだけで、Actor Turn Stateも格納拒否する。Debug/Serde/Chargeを一つのsolverへ統合していない。 |
| Futureの実引数とTask | check.rs:3422/:3439/:3452/:3514 | user async/aliasの実callee、argument index/type/Passing/BindingIdをprivate factsに保持する。spawnでは実argument payloadとTask outputのTxを拒否し、view/native reference拒否とTask受取義務を維持する。完了した子awaitの入力Txをouter unit argumentへunionしない。capture-free fnのTx署名だけを禁止理由にしない。 |
| final check→seal | check.rs:4523、checked.rs:42/:68、checked_tests.rs:432 | 最終check factsを渡し、同checkerをAST cloneへ再実行した結果との完全一致を要求する。欠落factsを補完・上書きして成功させる経路ではない。missing、wrong callee/index/Passing/BindingId、stale argument/call spanをmutation testsで拒否する。seal後AST/factsはprivate immutable。 |
| sealed SQL | checked.rs:615/:1187、emit.rs:632、sqlite_public.rs:439/:529 | canonical操作だけがargument1 Static/Owned planを持つ。dynamic SQLはstr ToOwnedでworker境界前に所有化され、Parametersより先に評価される。plan再構築との比較で欠落/index/representation変更を拒否する。SQL Err/panic後にParametersを評価しないnative oracleもある。上のloan修正込みでcheckerとの寿命を揃えた。 |
| Failure field | stdlib.rs:1691、checked.rs:724、emit.rs:560、runtime/sqlite/mod.rs:52 | scalar kind/outcome/retiredはValue、messageはString.as_strのBorrowStr planである。古いnative fieldのmethod形式をSQLiteへ誤用していない。messageはownerのwhole-owner loanとなり、借用中moveの負例を維持する。 |
| 行型 | capabilities.rs:140、check/sqlite.rs:64、checked.rs:1257、sqlite_public.rs:439 | 新query/allは生成FromRow可能なscalar class/一層Optionだけをcheckerで受理し、u64・nested class等は拒否する。生成側と同じfield predicateを使う。旧Dbのclass-only/手書きRust FromRow経路は狭めていない。実データ型/NULL/数値範囲はnative decode Failureの責務である。 |
| opt-in SQL | sql_check/collect.rs:24/:32、engine.rs:191/:521/:559、mod.rs:329 | canonical新opだけ追加収集し、新Parameters countはNone（unknown）と明示する。literalのschema/必要列/readonly/rowless shape/一文/匿名placeholderを準備検査し、SQLはstepしない。exec準備DDLはschema snapshotを更新しない。既存固定bind数/numbered bind/旧shapeを維持する。offline sandboxの対応範囲とruntime authorizerのSQL受理範囲は同一保証ではない。 |
| 終端/Passingのruntime接続 | check/sqlite.rs:38/:101、runtime/sqlite/mod.rs:193/:231/:269/:282/:288 | Options/Parametersをmove、Pool/Txをreference、commit/rollbackはTxをFuture構築時からmoveする。生成SQLはSql値、Failure.messageはborrowed strとなりruntime public APIと整合する。Tx Drop/EOF cleanup責務をcompiler独自runtimeへ移していない。 |

same-checker replayは別owner solverの維持を避け、不一致拒否の根拠として妥当である。ただし新factsだけを照合するためにfinalizationごとにfull AST cloneとfull checker一回を追加する。欠落metadataをこのpassで修復して元ASTへ戻してはいない。追加passのコンパイル時間/peak memoryは今回のruntime cost harnessでは測定されない。将来最適化するなら、final factsの完全性拒否を維持し、compiler段階の時間/メモリを別に測るべきである。これは現時点の性能blockerを実測した指摘ではない。

## oracle・native・配布・費用の確認

- 新SQLite/既存Task runnerは生checker errorだけをfragment判定へ渡す。source excerpt/file名を判定入力にせず、その回帰oracleを追加している。parse/resolve/panicをchecker negative成功へ数えず、元fixture path/line照合も維持する。保存46入力は14 accepted/32 checker rejected、matched46である。
- 既存built runnerを用いた独立小実行でもSQLite46/46、Task148/148だった。結果は `compiler-review-oracles/sqlite-contract-check-independent.json` / `task-contract-red-independent.json`。この実行はparser/resolution/checker/original-lineまでで、native実行とは分ける。
- `sqlite_public.rs:68` の7 positive組はnative_tripleを3構文で通す。native_triple.rs:106でHigh削除後に保存Lowを再load、:122で生成Rustを変えずadapter/assertionsを追加し、Cargoが返した対象test executableを:216で実行する。結果が空mainの実行だけにはならない。全定義関数の任意body executionを証明するものではなく、04のdiscardはpointerとしてacceptへ渡すがacceptはcallbackを呼ばない。このoracleの保証はcapture-free pointerの転送である。
- `compiler-final-focused.log` 本文はlib127、registry4、SQL14、parser2、sqlite_public6 testsの成功。`compiler-clippy.log` はnagic all-targets warnings deny成功。途中 `compiler-public-native-matrix.log` は失敗しており最終成功と扱わない。追跡alias native観測は上記別ログに分ける。
- `examples/sqlite_pool.nagi` は実SQL/Parameters/commit/read/rollback/closeを実行し、作業Errより先にclose結果を観測する。verify_sqlite_example.pyは日英コードblockのexact一致とHigh/High削除後saved Lowのcheck/build/run・stdoutを確認する構造である。完成public Docs本文のreviewは別followup。
- releases/verify.py:138の検査はarchiveから展開したexeを呼び、NAGI_ROOTを外したnative生成manifestがexe隣のruntimeへ向くことと7/closed出力をassertする。3mock oracleはgate/path/source消失の検査でありnative実行証拠ではない。immutable archiveそのものの実検証は親が担当する。
- sqlite-public costは同一public runtime/Options/active Tx/SQL ownershipでgeneratedとmanualを比較する。32小sample、4warmup、直接変更数、rollback/close、calling-thread allocation、未poll Future sizeを記録する。bare rusqlite/旧Db/全heap/worker allocationとの比較ではなく、performance thresholdもない。測定結果・依存lock同一性はこのsourceレビューから推測せず親の実観測で確認する。

## raw evidenceと未確認範囲

| 原ログ | SHA-256 |
| --- | --- |
| compiler-final-focused.log | `5491a05ef519736fd8797faacc03d4f18975ce552e79f47264202894d1e98c13` |
| compiler-clippy.log | `d527ea2381d5e7541effa6764e5c65871a6d85fb2b4584fe2d60bc47326f5332` |
| compiler-public-native-matrix.log（途中RED） | `18c03134e1702e99e143a3637a77582593dd2f93331e75632066ee23e5fb6def` |
| contracts-check.json | `b1169b7977b6c0499cac835e1ff66d803c2e14f49b5f4e834dee87d9238b0927` |

重いCargo/native再実行は共有targetでの親の作業と分け、本レビューではstatic source/契約/raw logと小さい通常checker/finalization oracleを使った。変更後の最終全suite/clippy/fuzz、生成探索、4 OS、public Docs/CI、cost結果、immutable配布物は本メモで完成保証していない。runtimeの全interleaving、trusted Rustの最終traits/link/target、実DB差、全OOM/abort/process kill、non-yielding処理の完了はこのcompiler reviewの保証外。巨大allocation、負荷、security PoC、第三者targetの再現は行っていない。

このメモは後続の修正を含むcompiler scopeのreview結果であり、merge/release/version変更やPhase全体acceptanceを承認するものではない。
