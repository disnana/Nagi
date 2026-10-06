# 言語とランタイムの契約

この文書はNagi処理系を変更する際の判断基準である。現行コードを仕様の正解とみなさない。契約を変える場合は、変更理由・互換性・例・検査を一緒に更新する。

「契約」は維持する意味論、「委譲」はRustや外部環境で確かめる条件、「制約」は現在対応しない範囲を表す。テストは挙げたケースを継続検査するもので、全プログラムについての証明ではない。

## 採用方針と現行契約の区別

2026-10-06の引継ぎを[ADR 011](adr/011-language-behavior-and-docs.md)へ取り込んだ。各節の現行契約表は作業branchの実装を表す。S1のTask契約は別節に分け、旧Scopeへ上書きしない。OWN-04は追加の承認を受け実装済み・未リリース。spawn結果handleと子業務Errの扱いを変えるASYNC-03/04は[ADR 012](adr/012-task-result-handles.md)に沿いmainへ接続済み・未リリース。条件付きshared messageのACTOR-01は方向のみ採用で、ACTOR-01は現在有効な受理規則ではない。

変更する際はbefore/after、互換性と対象版、High/Low、診断位置、生成Rust、実runtimeの成功・失敗・取消を検査する。移行前の暗黙代入moveは監査記録に残し、未変更の引数・return等のconsumeとScope子Errの契約を保つ。Supervisorのterminal failureをHTTP停止へ伝える接続も保つ。未決の細部は[Q-005〜007](open-questions.md#q-005-既存所有値の代入を明示する範囲)にまとめる。

明示moveの実装と通常代入の移行は、作者の追加指示で確定した。次のOWN-04を先行テストと実装へ反映した。公開版への反映と、このbranchの実行検証は[実装結果](explicit-move-results.md)で分けて記録する。task handleと故障型は別工程である。

### OWN-04: 明示moveの確定仕様（実装済み・未リリース）

- `std.ownership.move(value)`は引数一つ、戻り型は入力と同じ。明示型引数は不要かつ不許可。import/alias/qualified名はcanonical operation identityで解決し、ユーザーの同名関数を操作として扱わない。
- 所有する非Copy値を消費し、その値・入れ子のview origin・cleanup責任を一度だけ転送する。clone、allocation、共有所有者の追加、closeやrollbackは行わない。Copy入力は既存のCopy規則に従う。
- 代入の右辺が解決済みの所有ローカルそのもので非Copyなら、宣言・型注釈・再代入を含め通常代入を拒否する。括弧だけで包んでも同じ。borrowed loop localは所有者ではなく、従来の読み取り専用制約で拒否する。
- 新規生成値や関数呼出しは通常束縛できる。引数、return、field/index、try、matchの既存consume規則を一括変更しない。moveを加えても借用中の所有者移動、shared/borrowed親の非Copy field取得、局所viewのescapeは許可されない。
- Copy対象は既存`copy_type`とcanonical resource descriptorに従い、サイズやRustの推測から決め直さない。viewはCopyでも借用元の制約が残る。shared/ResultはpayloadがCopyでも非Copy。対応済みのローカルasync関数別名もCopyであり、関数値のasync provenanceを維持する。Futureや入れ子のFutureをこの操作で受け渡す機能は追加しない。
- move後の使用はNagi位置で拒否し、再初期化で新しい値を受け取った後は再利用できる。分岐・loopの既存join/fixed pointを保つ。Highと保存・手書きLowで同じ意味にする。

移行前の受理と実行は[監査結果](value-task-audit-results.md)に保存する。新仕様はテストのoracleを明示的に変更する承認済み契約であり、既存実装へ合わせて拒否を緩めない。[実装計画](value-task-implementation-plan.md)に順番と検証条件を記録する。

通常の引数と両側を評価するoperandの左から右の順序、and/orの短絡、値とcleanup責任の移動を保つ。逆順cleanupの方針は単純な同一ブロックの所有ローカルの逆宣言順を指し、全値の生成時刻逆順ではない。再代入・一時値・部分move・field/List/shared/Futureの規則、取消要求と終了確認は別にする。CheckedProgramは静的factsとRust生成planの境界で、runtimeのI/O成功や全backendの意味同値を証明するものではない。

## コンパイラ

| 項目 | 契約・境界 | 実装と検査 |
|---|---|---|
| parse | 対応する構文・型の形・入力上限を満たす入力をASTへ変換する。parse成功は型や所有権の受理ではない。不正入力は診断で失敗し、panicで終わらないことを目標にする | `compiler/src/lexer.rs`, `parser.rs`; `compiler/tests/frontend_contracts.rs`, `literal_contracts.rs` |
| check | 名前、Nagi型、move、view origin、Result、制御構造を検査する。通常のcheckはCargoを起動しない。エディターのエラー回復ASTはcheck成功の証拠ではない | `check.rs`, `modules.rs`; `ownership_boundaries.rs`, `typed_errors.rs`, `symbols.rs` |
| check→codegenの封印 | 最終Low/native統合・checkと生成plan確定を通したCheckedProgramだけをRust生成へ渡す。外部構築・可変化・未検査Programの生成を許さない。必須型・名前・operand・storage factsの欠落を再check/defaultで修復しない | `check/checked.rs`, `emit.rs`; API compile-fail、`check/checked_tests.rs`、conformanceのfinalize stages。Phase 1の変更 |
| check→build | サポートするNagi機能の受理後、Nagi側で検出可能な型・所有権・寿命の問題で**コンパイラ生成Rust**が拒否されるのは不具合。rustcの拒否を「追加の安全確認」として隠さない | `emit.rs`, `view_flow.rs`; conformance、`view_flow_foundation.rs`, `view_flow_completion.rs` |
| Rustへ委譲 | 手書きRustの本体、外部crateのAPIとtrait実装、最終的なClone/Send/Sync、依存取得・link・target設定をbuildで確かめる。この委譲をNagi自身の型生成ミスの免責に使わない | `rust_dependencies.rs`, `build_diagnostics.rs`, `copy_capabilities.rs`, `scoped_tasks.rs` |
| High/Low | 同じ名前解決・型・所有権規則を使う。Lowに別のメモリモデルはない。保存Lowを再解析して受理でき、対応するプログラムの観測結果が一致することを検査する | `parser.rs`, `check.rs`, `modules.rs`; `ownership_boundaries.rs`, `stdlib_imports.rs`, conformance |
| 診断 | Nagiで分かる誤りはNagi位置へ返す。生成Rustのprimary診断は対応がある時だけ元ファイル・行を示す。Rustの列・補足spanを推測してNagi位置に変換しない | `source.rs`, `diagnostics.rs`; `build_diagnostics.rs`, `symbols.rs` |
| ビルド成果物 | 別の入口ソース・生成先のアプリを共通targetへ置いても、片方の実行ファイルをもう片方で上書きしない。Phase 2では同じapp IDの再buildも世代を分け、旧成功exeを上書き・削除・killしない。協調するwriterをcanonical out単位で直列化し、Cargo・公開・互換出力の更新後だけlatestを置換する。run前にlockを解放し、選択したexe pathを保持する | `generation.rs`, `emit.rs`; `build_generations.rs`, `shared_target.rs`, `project.rs`; [ADR 007](adr/007-build-generations.md)。Phase 2の開発差分。公開版への反映状況は[進捗](progress.md)を参照 |

buildには外部環境が必要なため「check成功ならどんな環境でもbuild成功」とは保証しない。未対応のNagi構文・型の組合せは早い段階で明示的に拒否する。現時点では全受理プログラムのbackend conformanceを証明できておらず、未知の不一致は残り得る。

登録resourceのCopy/shared/field storage/Debugと型引数の保持関係は、[ADR 008](adr/008-resource-contracts.md)の単一根拠へ集約する。これはPhase 3の採用設計で、現受理意味論は変えない。inline/shared payload、Actorの間接protocol、callback署名、nominal phantomを区別し、全capabilityへ同じ遍歴を使わない。lifecycleは不活性のUnspecifiedに留め、Tx/Poolや任意Rustの安全契約を実装済みとしない。

## 所有権・借用

| 項目 | 契約・制約 | 主な検査 |
|---|---|---|
| move / owned | 非Copy所有値を渡す・返すと所有権が移る。所有する非Copyローカルそのものの通常代入は拒否し、`std.ownership.move`で明示する。新値生成とfield/index等の既存consume規則は維持する。move後の使用を拒否する。`owned[T]`は`T`との暗黙変換ではない。一般的なowned constructorや透過的演算は未対応 | `explicit_moves.rs`, `ownership.rs`, `owned_copy_codegen.rs`, `result_discard.rs`, `ownership_calls.rs` |
| view / aliasing | viewは読み取り用の借用。借用元が必要な間、そのplaceまたは親のmove・上書き・変更を拒否する。別フィールドは区別する。独自の実行時寿命検査を加えない | `view_origins.rs`, `ownership_boundaries.rs`, `iterator_borrows.rs` |
| shared | `shared[T]`は所有権を共有する。共有親から非Copyフィールドをmoveできない。Arcのcloneとpayloadのcopyを区別する。内部資源まで不変になる保証はない | `shared_field_moves.rs`, `comparison_ownership.rs`, `copy_capabilities.rs` |
| field access | 所有classの部分moveを追跡する。borrowed/shared親から所有フィールドを取り出すなら明示的copyが必要。型の表示名だけで親の所有形態を決めない | `class_field_types.rs`, `shared_field_moves.rs`, `ownership_boundaries.rs` |
| reborrow / sequential borrow | viewの引き継ぎは元のoriginを維持する。局所値への一時的な借用を退役させて入力借用へ戻す場合、生成Rustに不要な長寿命を結び付けない。現checkerはブロックを基準とする保守的な規則で、Rust NLLと同じ受理集合ではない | `view_parameter_rebinding.rs`, `view_branch_rebinding.rs`, `view_flow_foundation.rs`, `view_container_drop.rs` |
| branch | 継続する全経路のmove/borrow状態を統合する。終了経路を後続joinへ混ぜない。条件式・短絡の副作用と評価順を維持する | `loop_ownership.rs`, `view_branch_rebinding.rs`, `view_flow_foundation.rs` |
| loop | 0回実行、反復、条件の最後の評価を考慮する。反復で再利用する所有値は全継続経路で復元する。初回だけのchecker factsを固定点後の意味として使わない | `loop_ownership.rs`, `list_borrow_iteration.rs`, `view_flow_completion.rs` |
| function value | 引数由来のviewを返す関数値は実引数のoriginを引き継ぐ。借用元引数がないview返却はstaticだけ。関数値経由でlocal viewをstaticへ格上げしない | `static_callback_views.rs`, `borrow_free_returns.rs` |
| async / return | returnするviewは許可された入力またはstaticに由来する。local所有値へのviewを外へ返せない。awaitを跨ぐ値の破棄と取消はRust Futureに従う。spawnへ局所借用を逃がせない | `async_value_types.rs`, `scoped_tasks.rs`, `view_flow_completion.rs`, `view_container_drop.rs` |
| static view | 標準APIやextern宣言の明示的staticなviewを終了まで有効として扱う。Nagiの文字列literalは所有strであり、`view("local")`だけでstaticへ格上げしない。local変数や所有引数もstatic扱いしない | `static_callback_views.rs`, `borrow_free_returns.rs` |
| 再代入・破棄 | 右辺を評価して新値を元のcleanup anchorへ置き、その後旧値を退役させる。右辺が旧値を消費しなかった場合、そのerror/panicで旧値の破棄責任を失わない。右辺が旧値を明示moveした場合は移動先に責任が移り、巻き戻さない。旧値のDropがpanicしても新値の破棄責任を失わない。暗黙clone・追加payload allocationで回避しない | `view_container_drop.rs`; `emit.rs`, `view_flow.rs` |

候補の私有storageに`Option<T>`を使っても公開型は`T`のまま。内部slotの空状態はユーザーが観測するnullableではない。空slotからの読み取りが受理経路に現れた場合はコンパイラの不具合である。

## エラー

| 区分 | 契約 | 根拠 |
|---|---|---|
| Result | 期待する失敗を値として扱う。`try`はErrを呼び出し元へ返す。独自class/enumのEを使える。Resultを裸の式として捨てない。ownedで包んでも捨て忘れ検査を回避しない | `compiler/tests/result_discard.rs`, `typed_errors.rs`, `result_stdlib.rs`; `runtime/src/result.rs` |
| nullable | `T?`/Optionは値の不在。Err、panicとは異なる。matchでSome/Noneを処理する | `option_match.rs`, `nullable_roundtrip.rs` |
| compile error | Nagiの契約違反・未対応機能・無効入力。実行時Errへ自動変換しない | `frontend_contracts.rs`, conformance compile-fail |
| runtime error | DB・HTTP・I/OなどのAPIがResultで返す失敗。処理、変換、伝播をアプリが選ぶ | `runtime/src/lib.rs`, `database.rs`, `http_server.rs` |
| panic | 範囲外アクセスなどの異常。Resultへの普遍的な自動変換はない。catchできる境界でunwindだけを捕捉する。abort、OOM、プロセス障害は別 | HTTP panic tests、scope/actor tests |
| infrastructure error | Cargo/rustcの不在、依存・link・ファイル・権限・targetの失敗。Nagiの型エラー、業務Err、ICEと分けて報告する | `installation.rs`, `rust_dependencies.rs`, `build_diagnostics.rs` |

整数の除数をliteralの0と書いた場合はcheckで拒否する。型検査後の共通`constant_eval`は、8整数幅の副作用なし式と小さいscalar binding factsを扱い、compound/alias zeroとsigned MIN/-1・MIN%-1も拒否する。到達不能な枝も検査する。型・名前の誤りは追加の定数診断より優先する。

`+/-/*`とMINの単項否定のoverflowは従来どおりdebug panic/release wrap。このprofile依存の値は定数として伝播しない。関数、extern、field/index、任意の代数簡約は評価しない。loopが書くbindingは条件の評価前からUnknownにし、同じKnownを保つ継続枝だけをjoinする。Unknownを「安全に実行できる」という意味に使わない。実行時にしか分からない整数ゼロ除算はpanicとなり、floating-pointには整数の禁止規則を適用しない。

根拠: `compiler/src/constant_eval.rs`, `compiler/tests/constant_validation.rs`, `integer_zero_division.rs`, `integer_arithmetic.rs`。[ADR 002](adr/002-constant-validation.md)に診断拡張の互換性と不採用案を記す。codegenは定数foldを再実装せず、signed MIN leafの印字だけを正規化する。

## 認証・認可の最小境界

未リリースの実験。`std.auth.Principal`と`Grant[P]`は登録済みopaque resourceで、通常classの構築・JSON復元とは分ける。`P`は解決済みclass/enumのnominal IDで、型引数を持たない。proofはnonCopy/nonClone/nonSerdeで、nested wrapperからもcopy/shared化できない。class/enum fieldへの格納も拒否する。localの所有Option/Resultによる移動とasyncへのowned delegationは許す。

保護externの署名がGrant[P]を要求する場合、その権限型の値を渡し、move後に再利用しないことをcheckする。保護adapterは消費したGrantのresource IDで処理し、別のbare IDへ権限を付け替えない。署名を実際に守ること、verifierとNagi/Rust policyの正しさ、expiry/revocation/DB競合への対応はtrusted adapterとアプリの責任である。

全routeの保護、任意SQLのtenant制約、DTO流出、任意Rustの迂回はこの型モデルでは証明しない。普通のclassは入力・claims・errorとして有効だが、存在だけでは認証の根拠にならない。JWSの暗号処理を独自に実装せず、既存Rust verifierを接続する。初回の固定credentialデモはJWS verifierを実装していない。

根拠: `runtime/src/auth.rs`, `compiler/src/stdlib.rs`, `capabilities.rs`, `compiler/tests/auth_boundaries.rs`。[ADR 001](adr/001-backend-boundaries.md)に実験の範囲と信頼境界を記す。

## async・runtime

| 項目 | 契約・限界 | 実装と検査 |
|---|---|---|
| task lifetime / scope | scope本体の終了後に子をjoinする。本体の実行中、子の失敗で本体へ割り込まない。join時の子のErr/panicまたはbodyのErrでは兄弟を取消し、通常のエラー出口は破棄完了を待つ。所有者Dropはabortを要求するが同期Dropだけでjoin完了を保証しない | `runtime/src/concurrent.rs::Scope`, `compiler/tests/scope_runtime_contract.rs`, `scoped_tasks.rs` |
| cancellation | Futureの取消は所有資源をRust Dropで片付ける。既に開始したblocking/DB処理や外部副作用は巻き戻らない。yieldしない処理を期限時刻で強制停止できない | `runtime/src/concurrent.rs`, `database.rs`; HTTP deadline tests |
| actor | mailbox admission、reply、業務Err、worker failureを区別する。受理済み仕事はcaller timeout後も実行され得る。replyのErrとhandler自体のErrは別 | `runtime/src/actor.rs`, `actor/tests.rs` |
| supervisor | restart policy、回数・時間窓、shutdownの規則を持つ。最後のworker終了や強度超過はterminalとなり、同scopeの兄弟へ取消が伝わり得る。業務replyのErrだけで再起動しない | `runtime/src/actor/lifecycle.rs`, `actor/tests.rs`, `actor/lifecycle_adversarial_tests.rs` |
| cleanup / shutdown | 子・context・reply・admission permitの破棄責任を所有者に持たせる。取消とshutdown後に予約やtaskが残らないケースを検査する。任意Rust destructorが停止しない、panicする、外部資源を破損する場合の普遍的回復は保証しない | actor adversarial tests、`concurrent.rs` tests、HTTP shutdown tests |

Tokioのtask/Future、RustのDrop、Arcを利用する。BEAMのVM、分散監視、無停止コード更新、プロセス障害からの復旧は提供していない。

### S1 Task結果handle（main・未リリース）

[ADR 012](adr/012-task-result-handles.md)と[接続判断](task-handle-implementation.md)に沿い、SpawnBind・canonical std.task metadata・私有ScopeIdとbinding義務・sealed受取/放棄/scope plan・public runtimeを接続した。[接続結果](task-handles-s1-results.md)でchecker、native、CI、測定の範囲を分ける。[Stage 1](task-bridge-stage1-results.md)は接続前の歴史的記録である。旧spawn-only Scopeは変更しない。

| 項目 | 作業branchの契約 |
|---|---|
| handle / scope | Taskは非Copy・非Clone・非shared。作成時scopeのlocalに限定し、scope外・関数・field/container/wrapper・他taskへのescapeを拒否する |
| consume | TがCopyでもawaitで一回だけconsumeする。全T正常binding/scope出口とloop継続でawait/discardを必要とし、move aliasへ義務を移す。異常退出はcleanup。一般owned/内側Result bindingのmust-useは追加しない |
| 値と故障 | actual join後の外側Result[T, TaskFailure]。TがResultなら入れ子を保ち、普通の業務Errで兄弟を止めない |
| scope fault | panic/予期しない取消/legacy Err/protocol故障は最初に観測したprimaryをsticky保持。兄弟abort要求→actual drain後に外側Err。受取Err処理後もscope出口Err、bodyの元Eを後続faultで置換しない |
| owner / 取消 | 唯一JoinSet owner、scope内ticket、未join native ID対応。join Ready→record間にawaitを挟まない。receiptはScope強参照を持たず、受取Future取消でhandleを復活させない。Scope同期Dropはabort要求まで |
| 放棄と保持 | discardやTのDropをactual join/close成功と数えない。完了未join task・未受取Tの保持とallocation/retireを検査し、実行中数だけでメモリを説明しない |
| 既存連携 | S1では旧spawn fail-on-ErrとSupervisor terminal→HTTP取消を維持。S2は親がmonitor内側Resultをtryする移行。新fault昇格APIは追加せず、公開Pool/Txは別工程 |

TaskFailureはopaque・非Copy・非Clone・非shared、kind()はCopyな四値enum、message()はFailure-origin view。Task bindingを含む最寄りscopeのみTaskScopeを選び、nested scopeは独立する。元cleanup anchorとbodyラベル、Error変換、評価順を維持する。保証範囲と未確認targetは[接続結果](task-handles-s1-results.md)へ記録し、公開Pool/Txやreleaseの保証へ広げない。

## DB

| 項目 | 契約・境界 | 実装と検査 |
|---|---|---|
| SQL static check | `check --sql-schema … --sql-dialect sqlite`の指定時だけ、対象builtinの直接literalをprepareし、名前・必要返却列・bind数・文種別を検査する。queryは実行しない | `compiler/src/sql_check/mod.rs`, `compiler/tests/sql_check.rs` |
| schema assumptions | 1つの指定schemaが検査対象DBに一致することを利用者が管理する。アプリのmigrationや複数Dbからschemaを推測しない。schema処理の権限・入力・時間を制限する | `sql_check.rs` tests、[公開リファレンス](../sql-check.md) |
| NULL / type mismatch | 必要列があっても値型・範囲・NULL・実DBの状態は実行時まで分からない。nullableフィールドのNoneと非nullableの読み取り失敗を区別する | `runtime/src/database.rs`, `compiler/tests/nullable_database.rs`, `owned_database.rs` |
| transaction | 標準のtransaction ownership APIは未実装。共有Dbへ複数のBEGIN/query/COMMITを送っても、呼出し間の排他所有は保証しない | `runtime/src/database.rs`; [DB制約](../database.md#実装と制約) |
| cancellation | 送信待ちと受理後を区別する。受理済みSQLite jobはcallerの取消後にも完了・commitし得る。最後のDb所有者のDropはworker終了を待ち、即時終了ではない | `database.rs::call`, `Inner::drop`; database tests、公開DB制約 |

PostgreSQL、一般的な可変長bind、poolは未実装。Rustアダプター経由の独自実装を標準機能の保証と混同しない。

### Phase 4で採用した契約（実装・検証中）

[ADR 010](adr/010-sqlite-transaction-boundary.md)と[API契約](sqlite-pool-proposal.md)は2026-10-06に承認済み。旧Dbの契約を変えない。TxのnonCopy／nonshared／field保存・task転送禁止、終端consume、cleanup確認前の再利用禁止、結果不明とcleanup failureの分離、close後のclosing維持を採る。関数pointer署名と実捕捉、native Dropとrollback成功、close通知とworker joinを区別する。SQLiteのSQL解析・native Txはrusqliteへ委譲する。private試作の成功を標準APIの保証へ広げず、public checker／生成／実DB／4 OS acceptanceまで未検証範囲を記録する。

Q004で[capability初版表](sqlite-pool-adapter-decision.md#registry配線前に固定するcapability)を採用した。Poolは明示clone、Tx／ParametersはnonClone。Tx／ParametersのDebugは禁止し、Pool／FailureのDebugは状態のみ。Failureと小さいenumはshared可、全新resourceのSerdeと新Actor Charge対応は不可。関数署名やmarkerを実payloadと混同せず、Txの永続格納・task転送禁止をnative inline stateにも適用する。これらは公開配線時の契約で、private試作がcheckerで保証したという意味ではない。

statement Errから「変更0」や「rollback済み」を推論しない。SQLiteの`OR FAIL`やAFTER triggerでのstep失敗は先行効果をactive Txへ残し得る。禁止actionの拒否とTx rollback成功を別oracleで検査する。普通のErrでの継続可という採用契約を、暗黙savepointや全Err自動abortへ変更しない。

取得予算はlogical slot待ちからnative record登録まで共用し、登録後のready／BEGIN／SQLへ持ち越さない。0msは即時の空きを利用できる指定。取得期限切れだけでPoolを故障・退役・closingにしない。同taskの別取得と取消で予算を混同しない。[private設計](sqlite-acquire-budget-design.md)で先行検証し、公開Options／Poolの保証とは区別する。巨大capacityのstock allocationは[公開化前の判断](sqlite-capacity-decision.md)に残す。

## HTTP

| 項目 | 契約・境界 | 実装と検査 |
|---|---|---|
| handler Result / mapper | Okをresponseへ、ErrをAppの既定mapperまたはroute mapperへ渡す。業務Errをpanicと同一視しない | `runtime/src/http_server.rs::route_mapped`, `compiler/tests/error_routes.rs`, `tests/http_stdlib_integration.py` |
| panic | handler生成・poll・mapperのunwindをレスポンス開始前の境界で捕捉し、payloadを含まない500を返す。共有状態・DB・lockは巻き戻さない | `runtime/src/http_server/panic_tests.rs`, `runtime/src/http/panic_tests.rs`, `compiler/tests/http_entrypoint.rs` |
| malformed / limits | Hyperのprotocol解析とNagiのbody/header/capacity/deadline制限を区別する。全不正TCP入力に整形式HTTP応答が届く保証はない。過大本文は無制限drainせずclose戦略を使う | `runtime/src/http_server/tests.rs`, `runtime/src/http/tests.rs`, `runtime/src/http.rs` |
| timeout | body待機、handler、sendを分ける。handler期限切れはFutureを破棄してcapacityを解放する。non-yielding pollは期限を越え得るが、後で返ったReadyを正常responseとして採用しない | `http_server/tests.rs::handler_timeout_*` |
| response開始後 | 標準responseはbuffered。送信開始後のsocket失敗は接続終了になり得る。二度目の500や、送信済みbyteの取消は保証しない。streaming/upgradeは未対応 | `http_server.rs::BufferedBody`, `TimedIo`; shutdown/send tests |
| cleanup / shutdown | request permit、handler Future、connection taskを期限とshutdownで解放する。graceful shutdownの完了とdeadline超過によるabortを分ける | `http_server/tests.rs::graceful_shutdown_*`, `connection_capacity_and_shutdown_deadline_leave_no_handler_tasks` |

標準HTTPはHyper上の実装であり、既存Rust/Axumルーター用の経路もある。Axum全面移行、TLS、WebSocket、peer/proxy信頼APIは今回の契約ではない。

`axum-service`サンプルは別のRust adapter契約を持つ。[ADR 009](adr/009-axum-rejected-body.md)で承認した欠落Content-Typeの4096バイト・1秒読取は、この例だけに適用する。415の選択を全transportでの受信保証とせず、標準HTTPや他のRust adapterへ暗黙に適用しない。実装・検査の状況は進捗に記録する。
