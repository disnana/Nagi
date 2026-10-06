# Compiler conformance / regression / fuzz smoke

Nagi 0.1 betaの既知不具合再発を小さな再現sourceと段階別oracleで防ぐ。仕様判断は explicit contracts > accepted decisions > DESIGN > reference > tests > code。現在のcheckerのacceptだけを正例の根拠にはしない。

## 分類と実file

| 分類 | 必須の観測 | 主な実file |
| --- | --- | --- |
| Unit | lexer/parser/checkerの局所契約 | compiler/src/tests.rs, compiler/tests/enum_parser.rs |
| Compile-pass | checker受理後に実Rust compile成功 | compiler/tests/conformance.rs, compiler/tests/copy_capabilities.rs |
| Compile-fail | 指定stageで拒否、panicを成功扱いしない | tests/conformance/*negation*, *temporary_view*, *shared_field_move*, *owned_result_discard* |
| Diagnostic | 期待診断意味と元source line | compiler/tests/build_diagnostics.rs, compiler/tests/sql_check.rs, conformance.rs |
| Sealed API / facts | 未検査Programの生成・外部構築・可変化を拒否。check後の必須facts欠落と封印済みplan欠落を再checkせず検知 | compiler/src/emit.rs / check/checked.rsのcompile-fail、check/checked_tests.rs |
| High-Low | 保存Lowと手書きLowの受理・観測同値 | compiler/tests/frontend_contracts.rs, view_flow_completion.rs, conformance.rs |
| Backend | emit成功後の実rustc/Cargo build | compiler/tests/codegen.rs, http_entrypoint.rs, conformance.rs |
| Runtime | 値・byte列・Drop・panic unwind | compiler/tests/literal_contracts.rs, view_container_drop.rs |
| Integration | 実Cargo/extern/socket/SQLiteとNagi位置 | compiler/tests/shared_field_moves.rs, static_callback_views.rs, sql_check.rs; runtime/src/http/panic_tests.rs |
| Build artifact identity | 同じcacheの別アプリを取り違えない。Cargo終了後に別buildを挟む決定的barrier、既定・明示共有、等価source/outと別out | compiler/tests/shared_target.rs, project.rs; [ADR 005](adr/005-native-artifact-identity.md) |
| Build generations | 同一outのOS lock・待機通知、旧exe継続、成功snapshot/latest、Cargo/投影/置換失敗、孤児Cargo、各世代を読むreader。Windowsのrename失敗を別に観測 | compiler/tests/build_generations.rs; [ADR 007](adr/007-build-generations.md) |
| Artifact consumers | metadata不正・消失・未公開世代でcacheへ逃げず、成功artifactを選ぶ。旧世代方式より前の成果物だけlegacy fallbackを維持 | scripts/test_native_artifacts.py |
| Adversarial | overflow/zero division、loop backedge、panic/取消 | compiler/tests/integer_zero_division.rs, scope_runtime_contract.rs; runtime/src/actor/lifecycle_adversarial_tests.rs |
| Fuzz | 任意text mutationのparse/check panic、check後Low/emit | fuzz/smoke.rs, compiler/tests/support/conformance.rs |
| Property / 限定differential | bounded生成と独立host oracle、High/保存Low二経路 | compiler/tests/support/conformance.rs, compiler/tests/conformance.rs |
| Planned input syntax | 未配線APIのHigh／手書きLow構文、matrix完全性、元行anchor。compile-pass／failや診断保証とは別 | compiler/tests/sqlite_contract_inputs.rs、fixtures/sqlite-contract/matrix.json |
| Private native contract | 実SQLiteのuser hook、終端／cleanup結果、取消、native close／worker join。public Pool保証ではない | runtime/src/sqlite_prototype/tests.rs、[結果](sqlite-session-results.md) |
| Private pool adapter | stock checkoutの所有、create取消、close競合、retire後replacement停止、native上限、結果公開順。単一接続prototypeの検査で、多接続・公開取得期限の保証ではない | runtime/src/sqlite_prototype/adapter_tests.rs、[結果](sqlite-adapter-results.md) |
| Private multi-connection lifecycle | 同じfilesystem DBの2接続、active AとBの独立join、startup取消、stock detach先行permit、terminal cause、observer/native起動不成立。公開Options/取得期限の保証とは別 | runtime/src/sqlite_prototype/adapter_tests.rs、[内部設計](sqlite-multiconnection-design.md)、[検証記録](sqlite-multiconnection-results.md) |
| Private acquisition budget | Immediateの空き／logical不足／native不足、native待機中の有限期限、同absolute予算の引継ぎ、同task scope／取消、登録後startupとBEGIN、failure cause。公開Optionsの配線とは別 | runtime/src/sqlite_prototype/acquire_tests.rs、[設計](sqlite-acquire-budget-design.md) |

### Negative testの領域

現在のtest fileを次の分類で読む。分類のためだけに空directoryを作らず、実harnessと拒否段階を結び付ける。

| 分類 | 実harnessの例 |
|---|---|
| valid / invalid_type | conformance.rs, expression_contracts.rs, operator_types.rs |
| invalid_move / invalid_borrow | explicit_moves.rs, ownership_calls.rs, shared_field_moves.rs, view_origins.rs |
| invalid_null / invalid_result | option_match.rs, nullable_roundtrip.rs, result_discard.rs |
| invalid_async | async_value_types.rs, scoped_tasks.rs |
| invalid_auth / invalid_authz | auth_boundaries.rs: missing/fake Principal、wrong permission、proof再利用・共有 |
| invalid_rust_interop | build_diagnostics.rs, rust_dependencies.rs: adapter不一致とnative/依存側の拒否 |
| invalid_sql | sql_check.rs: schemaを明示した列・bind検査 |
| diagnostics / source_mapping | constant_validation.rs, build_diagnostics.rs: code・元module行・primary/関連位置 |

定数検査は`E_CONST_ZERO_DIVISOR`と`E_CONST_SIGNED_DIV_OVERFLOW`を固定する。既存型・ownershipとAuthの拒否は現在messageの意味とsource file/lineで検査し、全診断に安定したcodeや式spanがあるとは説明しない。Authのwrong permissionは通常の型不一致であり、route全体を解析する`NAGI-AUTH-001`は未実装。

## 実行

```sh
python3 scripts/verify_compiler_contracts.py
cargo test --locked -p nagic --test conformance
cargo run --locked -p nagic --example fuzz-smoke
# 登録検査だけではHTTP/SQL/Cargo統合の保証にならない。実harnessを順番に実行する:
python3 scripts/verify_compiler_contracts.py --run-linked
```

通常CIのconformanceは外部corpus42件＋限定生成24件。正常経路では全正例のHigh直接生成Rustと保存Low経由Rustをmoduleで隔離し、1回の `rustc --test` とnative実行にまとめる。High function callの `crate::` rootはcase moduleへ移し、生成string literal内のbytesを保持する。これはstd-only corpus用の隔離で、extern adapter/HTTP/SQLは専用harnessを使う。runtime依存を小runnerでstubして保証にしない。

`NAGI_CONFORMANCE_CASES` は1..2048（default 24）、`NAGI_CONFORMANCE_SEED` は1..u64::MAX（default 305419896）。`NAGI_FUZZ_MUTATIONS` は1..100000（default 1000）、`NAGI_FUZZ_CASES` は1..2048（default 16）、`NAGI_FUZZ_SEED` は1..u64::MAX（default 305419896）。不正なenv値は失敗とし、黙ってdefaultに戻さない。`RUSTC`でnative compilerを指定できる。

scheduleの例:

```sh
NAGI_CONFORMANCE_CASES=256 NAGI_CONFORMANCE_SEED=305419896 cargo test --locked -p nagic --test conformance
NAGI_CONFORMANCE_CASES=256 NAGI_CONFORMANCE_SEED=3735928559 cargo test --locked -p nagic --test conformance
NAGI_FUZZ_MUTATIONS=10000 NAGI_FUZZ_CASES=128 NAGI_FUZZ_SEED=305419896 cargo run --locked -p nagic --example fuzz-smoke
```

seedを保存し、失敗したcase indexを含む件数以上で同じcommandを再実行する。`NAGI_FAILURE_DIR` をCI artifact uploadの対象directoryに指定する（未指定ならOS tempの `nagi-conformance-failures`）。PRとscheduleは同じrunnerとoracleを使い、case数とseedだけを変える。

## Oracleと段階境界

`tests/conformance/corpus.json` はsource path、High/Low、compile-pass/run-pass/reject:stage、期待診断substring、期待line、native assertionを指定する。negativeは対象の初期parse/checkで拒否することに加え、診断意味とsource行も必須。panicや異なる段階での拒否をcompile-fail成功としない。正例はHigh parse/check → Low pretty → Low parse/check → High/Low各finalize・封印 → Rust生成 → rustc → 必要なnative実行まで全て必須で、後段拒否は保存して失敗する。finalize失敗の分類もartifactへ記録する。

生成は18種のaccepted bounded grammarを順番に使用し、seedで値を変える。i64算術/比較/list index/lenだけでなく、view copyと条件rebind、loop内local ownerから復元、List[view[str]] move/reinit、nested Result match、関数値、複数borrow sourceを持つResult/Option、最初のpollで完了する純async関数を含む。overflow、zero division、無限loopを作らない範囲を生成する。整数演算の期待値は独立host Rust計算、文字列長は明示byte数。明示moveの文字列/List/Result/Option・branch/loop再初期化5種も含む。High/Low両結果をこの期待値へ照合する。High/Lowは共通frontend/backendを使うので独立compiler間のdifferential testではなく、限定的なmetamorphic/観測同値検査である。純粋な生成にはsystem/environment依存や未対応owned[T]を混ぜない。

任意text mutationは初期parse/checkの通常拒否を許すがpanicを許さない。check成功後はLow再parse/checkとRust emit成功まで要求する。accepted件数とparse/check拒否件数を分けて報告する。任意mutationを大量rustcへ投げず、native段階は限定生成caseだけにする。以前同じsmokeに混ざっていたSerdeJSON mutationはNagi compilerの検証ではないので削除した。

## 失敗の保存と縮小

共通failure recordは `name/source/seed/stage/expected/diagnostic/high/oracle`。JSON、元source、`.min.source`、再計算したoracle付き`.min.case.json` を保存する。frontend panic/errorは現在stageを捕捉する。backend/runtime失敗は通常batchから最大32caseへ個別再現を探す。説明できなければname=batch、expected=rust-batch-run-passで実Rust batchを保存し、先頭negativeのsourceへ誤帰属しない。rustcは30秒、nativeは10秒のdeadlineでkill/reapし、出力はfileへ流してpipe詰まりを避ける。timeoutは個別再現/縮小を繰り返さない。

縮小はUTF-8の境界を守るchunk削除で、同じstageとdiagnostic signatureの再現だけを採用する。frontendは行番号を除いた診断内容、rustcは最初のerror code、native oracleの等値assert失敗はそのassert種別を保持する。診断が別のsyntax errorに変わったcandidateは採用しない。negativeは対象の拒否構文/元行も保持し、その行を削ってvalid programへ変える縮小を認めない。frontendは96試行、backend/nativeは8試行の上限。runtime失敗の生成caseはa/b/cパラメータだけを縮め、独立host期待値を再計算する。固定corpusのruntime sourceは意味保存を保証できないため任意削除しない。これはbudget内の縮小で、数学的な最小sourceを保証しない。再現しなければ元sourceをそのまま残す。seedは固定する。oracleは生成パラメータを縮小した場合だけ独立計算で更新する。

rustc段階の縮小は、oracleを除いた生成Rustだけでも元と同じerror codeで失敗する場合に限る。候補もoracleなしで検査し、テスト関数や期待値の参照先を削除したことで生じるE0425をコンパイラの反例にしない。oracle依存の失敗は元sourceを保持する。

self-checkはraw/byte raw/normal string・nested comment・identifierを守ったbatch root移動、hanging childのdeadline/reap、縮小器が余計な関数を除去できること、High checker failureをparse failureに変えないこと、およびoracleのE0425を誤った最小sourceへ縮めないことを検査する。新たな反例は仕様/accepted decisionで期待挙動を確定してから外部corpusへ追加する。テストを弱めたりunexpected backend rejectionをallowlistに追加して通さない。

## CIでの実行

PRとpushでは既存`Nagi checks`の変更検出を使う。compiler/runtime/test/configを変えた場合、Linuxの`cargo test --locked`にcorpus・property・診断・実統合が含まれ、fuzz smokeも実行する。4配布target（Linux・Windows・macOS Intel／Apple Silicon）の検査にはconformance・view storage・Drop・scopeの実テストを含める。Docs/サイト/AGENTSだけの変更はRust全検査を起動しない。

`Compiler contract exploration`は毎週月曜03:17 UTCと手動起動。2つの固定seedで256生成case、10,000 mutation、128 native case、重要なview/Drop回帰を実行する。各jobは30分まで。これは通常PRの必須gateを増やすworkflowではない。失敗時はPR/定期jobとも`build/compiler-failures/`をartifactへ保存する。

## Cargo / HTTP / SQLとの接続

`tests/conformance/harnesses.json` に実test名とcommandを登録する。`verify_compiler_contracts.py` は登録先source/testが存在することを検査し、`--run-linked` で17harnessを順番に実行する。HTTP panicは実request、500/sanitized body、HEAD body、server継続性まで検査する既存runtime harnessが責任を持つ。SQL missing-columnは実SQLite schemaのopt-in checkとHigh/保存Lowのquery行を既存SQL harnessで検査する。HTTP生成と成功build世代は実Cargo build/実行harnessへ接続する。conformance corpusへの文字列記録だけではこれらの性質を保証しない。

Phase 3では、登録資源の独立inventory、用途別capability、4例のLow/Rust全文goldenも接続した。固定logical ModuleIdのgoldenはresolver・生成の決定性を検査し、実fileのsource mapは既存統合testへ任せる。HTTPの追加native例は借用JSONとnamed mapperの登録構築を実行するもので、mapper本体を呼んだ証拠とはしない。4 OSの明示Cargo一覧にはinventoryと既存shared-field native回帰を追加し、既存HTTP/Actor/auth/copy検査も維持する。

## 一次資料と採否

- [rustc test infra](https://rustc-dev-guide.rust-lang.org/tests/intro.html) / [UI tests](https://rustc-dev-guide.rust-lang.org/tests/ui.html): check/build/runの区別と期待診断/位置を採用。rustc専用compiletestの直接依存、環境差を含む全面stderr snapshotは採用しない。
- [Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz.html) / [LLVM LibFuzzer](https://llvm.org/docs/LibFuzzer.html): 多様なcorpus、決定性、失敗保存/縮小を採用。現smokeはcoverage-guided fuzzではない。nightly/sanitizer/libfuzzer-sys導入は専用laneの検討として保留。
- [Proptest](https://proptest-rs.github.io/proptest/intro.html) / [generation・shrinking・persistence](https://proptest-rs.github.io/proptest/proptest/getting-started.html): property検査は既知regressionを補完する。今回は18種の小さなgeneratorと既存stdで縮小/保存を実測し、新dependencyを加えず実装できた。strategyの組合せが増え構造的shrinkingが必要になった段階でproptest dev-dependencyを提案する。
- [Csmith](https://embed.cs.utah.edu/csmith/): 未定義挙動を除く生成と独立oracleを採用。C言語generator自体は非採用。Nagiに独立compilerがないことを明記する。
- [Crater](https://rustc-dev-guide.rust-lang.org/tests/crater.html): check/build/runのコスト分離を採用。小corpusの成功を全言語/全platform保証と解釈しない。

## Axum連携サンプルのnative回帰

[ADR 009](adr/009-axum-rejected-body.md)のContent-Type欠落時のpolicyは、sampleのnative unitと元clientの実HTTPで分けて検査する。共通application runnerは、成功世代のmanifest・唯一のbinを使ってHigh/保存Lowそれぞれの`native::tests::`を実行する。0件・ignore・Cargo失敗は成功にならない。`test_application_native_tests.py`の5回帰もLinuxと4 OSのCIへ接続した。

制御read Futureのpoll/EOF・error・取消/Dropと、実Bodyの4096/4097、実handlerの分岐を確認する。実HTTPの19caseと不正port3caseは別に数える。分割送信と正常JSON4097byteの413を追加し、旧request/期待は維持する。全TCP分割・keep-alive・hard wall期限・clientへの必達を証明するtestではない。先行testのhelper未定義によるcompile失敗と、元Windowsの受信失敗を混同しない。

## 明示moveの回帰

`explicit_moves.rs`は既存非Copyローカルの通常代入を拒否する負例と、明示move・Copy・新規値の正例を対にする。canonical importを通常のresolverへ通し、元行と診断内容を確認する。実nativeの9経路はHigh・保存Low・手書きLowそれぞれで通常transfer、RHS失敗／破棄／取消、Copy集約値の比較を観測する。全操作のallocation数や全Future型を保証する検査ではない。先行失敗、既存fixtureの移行理由、検証結果は[明示moveの記録](explicit-move-results.md)に分ける。4 OSの明示Cargo一覧にもこのharnessを含める。
