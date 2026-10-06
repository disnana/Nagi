# ADR 011: 値・失敗・taskの方針と、現行Docsを分ける

状態: 2026-10-06の作者の引継ぎを採用。これは設計・文書の判断であり、未実装の構文や挙動を公開した記録ではない。

その後、作者が明示move・spawn結果handle・子taskの業務Errと故障の分離を段階実装する範囲を承認した。[実装順と具体案](../value-task-implementation-plan.md)に、現行との差、依存、Copy表、移行、先行テストを分離した。大枠は採用済みだが、その文書の具体APIと未決の意味論を承認済みにはしない。

監査基点: main `ff6f7d4c81c8cf49c2bca7abffb3083f681d5b9d`。添付が照合したmainと一致した。文書branchは`docs/python-guide-design-contracts`。初回監査時点ではPR #82のprivate Pool比較は別差分だった。後にmain `7999bab`への反映を確認し、文書branchへ統合した。公開Pool/Tx APIとして扱わない。

## 問題と採用理由

Python風の字下げだけを説明しても、参照代入、例外、taskの動作までPythonと同じだと誤解される。将来の書き味と、現在使える規則を混ぜることも同じ問題を起こす。

採用する方向は、読むならview、渡して手放すならmove、同じ値を保持するならshared。値の不在はnullable、処理の失敗はResult、通常の失敗ではない異常はpanicと分ける。scopeは子taskの寿命を所有し、actorの業務replyとworkerの故障を分ける。RustのDrop、Future、Tokioと既存crateで実現し、独自GCやasync destructorを追加しない。

入門は「やりたいこと → Pythonとの比較 → 現行Nagiの短いコード → 観測結果 → 間違いと直し方」。正確な型・拒否条件・取消の限界はリファレンスに残す。実装案のコードは動く例とは別に表示する。

## 方針と現状の対応

この表の「現行」はコードとテストの読み取り。今回実行した検査は[進捗](../progress.md)へ別に記録する。テストファイルの存在を今回の実行成功と数えない。

| 方針ID | 採用する方向 | 現行と残る差 | 根拠 |
|---|---|---|---|
| OWN-01 | moveで値と後片付けの責任を渡す | 非Copy値の代入・引数・returnには暗黙moveがある。moveはcloseではなく、再初期化後の利用は別 | [ownership](../../ownership.md)、[move検査](../../../compiler/tests/ownership.rs) |
| OWN-02 | viewは所有せず読む。元のplaceを必要な間保つ | 実装済み。検査はブロックを基準とする保守的なもの。全てのRust NLLケースを受理しない | [origin検査](../../../compiler/tests/view_origins.rs)、[呼出し内の借用](../../../compiler/tests/ownership_calls.rs) |
| OWN-03 | sharedは所有権を共有する | 実装済み。handle複製とpayload copyは別。内部資源まで不変になるわけではなく、Send/Syncは最終Rust検査も必要 | [shared field](../../../compiler/tests/shared_field_moves.rs)、[capabilities](../../../compiler/src/capabilities.rs) |
| OWN-04 | コピー可能な単純値以外の既存値の`a = b`は操作を明示する | **未実装・互換性変更**。明示moveの構文、Copy対象、view/shared handle、引数・return等への適用範囲は未決 | [checker](../../../compiler/src/check.rs)、[copy検査](../../../compiler/tests/copy_capabilities.rs) |
| OWN-05 | 通常の引数と両側を評価するoperandは左から右。短絡は不要側を評価しない | checkerと生成に順序を持つ。借用・引数・右辺置換のケースは検査済みのcorpusがある。全式の順序証明ではない | [引数](../../../compiler/tests/ownership_calls.rs)、[spawn引数](../../../compiler/tests/scoped_tasks.rs)、[置換・短絡](../../../compiler/tests/view_container_drop.rs) |
| ERR-01 | nullable、Result、panicを分ける | Some/Noneのmatch、Resultのmatch/tryを実装済み。任意のPython風Noneチェックでの型絞り込みはない。裸のResult破棄を拒否するが、未使用の変数へ代入したResultを全て検出するわけではない | [Option](../../../compiler/tests/option_match.rs)、[Result破棄](../../../compiler/tests/result_discard.rs)、[独自E](../../../compiler/tests/typed_errors.rs) |
| ERR-02 | panic捕捉は状態の復元ではない | HTTPの応答開始前のunwind捕捉を実装済み。abort/OOM/強制終了、変更済みstateやDBのrollbackは保証しない | [HTTP panic](../../../runtime/src/http_server/panic_tests.rs) |
| ASYNC-01 | awaitは結果を待つ。呼ぶだけで独立taskを作らない | async本体と引数評価・moveを分ける。第一級Futureの保存・返却・containerは未対応 | [async値](../../../compiler/tests/async_value_types.rs)、[生成](../../../compiler/src/emit.rs) |
| ASYNC-02 | spawnはscope所属の子を実行対象にする | 実装済み。別CPUでの同時実行や開始時刻は保証しない。spawnは文で、結果handleを返さない | [scope検査](../../../compiler/tests/scoped_tasks.rs)、[Scope](../../../runtime/src/concurrent.rs) |
| ASYNC-03 | 結果を一度受け取るhandleを用意する | **未実装**。現行spawnはunitまたはResult[unit, Error]だけ。型名、故障型、消費規則、scope外への持出しは未決 | [checkerのSpawn](../../../compiler/src/check.rs)、[JoinSet出力](../../../runtime/src/concurrent.rs) |
| ASYNC-04 | 普通の子taskの業務Errは値として受け取り、task故障とは分ける | **現行と相違**。Scopeは子のErrでも兄弟を取消し、通常出口では終了を待つ。Supervisor terminal ErrをHTTP停止へ伝える既存連携は維持して移行する | [実Scope契約](../../../compiler/tests/scope_runtime_contract.rs)、[Supervisor](../../supervisor.md) |
| ASYNC-05 | 取消要求と終了確認を分ける | scope本体終了後のjoinで子の失敗を検出する。本体実行中に割り込まない。親Future Drop/unwindでは同期Dropがabort要求を出すだけで、join完了を待てない | [Scope実装](../../../runtime/src/concurrent.rs)、[実Scope検査](../../../compiler/tests/scope_runtime_contract.rs) |
| LIFE-01 | 操作用handle、実行owner、終了確認APIを分ける | Supervisor ControlのDropだけでは終了しない。Arcの循環を自動回収しない。sharedだけで終了完了を保証しない | [Supervisor](../../supervisor.md)、[lifecycle](../../../runtime/src/actor/lifecycle.rs) |
| LIFE-02 | 同一ブロックの単純な所有ローカルは逆宣言順に片付ける | 一時値、再代入、部分move、field/List/shared/Futureの規則を一括で生成時刻逆順にしない。既存cleanup anchorとRHS→置換→旧値退役を維持する | [cleanupの観測](../../../compiler/tests/view_container_drop.rs)、[生成経路](../compiler-pipeline.md) |
| LIFE-03 | commitを明示し、未終端Txのcleanupをworkerが所有する | ADR 010承認済み。private一接続試作と公開Pool/Tx完成は別。取消、rollback結果、COMMIT outcome、native close、joinを別に観測する | [ADR 010](010-sqlite-transaction-boundary.md)、[試作結果](../sqlite-session-results.md) |
| ACTOR-01 | actor自身の状態はmessageで操作し、messageはmoveを基本にする | 現行message/replyはview/shared/native資源を拒否する。条件付きshared messageは**未実装**。共有DBなど外部資源の状態とは別 | [actor検査](../../../compiler/tests/actor_stdlib.rs)、[actor](../../actor.md) |
| ACTOR-02 | reply Err、worker Err/panic、callの外側のErrを分ける | TurnのreplyとhandlerのResult、callのResultを実装済み。reply Errだけで再起動しない。worker Errは再起動理由になる | [actor runtime](../../../runtime/src/actor.rs)、[actor reference](../../actor-reference.md) |
| ACTOR-03 | one-for-one、再初期化、再起動上限、明示停止 | policyで正常終了を区別する。最後のworker/強度超過はterminalになり得る。message再配送・exactly-once・BEAM隔離は提供しない | [lifecycle検査](../../../runtime/src/actor/lifecycle_adversarial_tests.rs)、[Supervisor](../../supervisor.md) |

## 意味を決める境界

Nagiは型、値の受け渡し、評価順、Result、対応する制御構文を定める。標準APIはscopeの終了や資源のclose成功条件を定める。checkerは静的に確かめたfactsを、CheckedProgramは確定した生成planとともに渡す。実行時のI/O成功や終了確認を静的な証明へ読み替えない。

CheckedProgramはRust生成用の私有planも持つ。backend非依存の完全な意味論IRではない。[ADR 006](006-sealed-codegen-input.md)は維持する。Lowは中間的な構文でもあるが、安定したbackend非依存IRとしては保証しない。High→Low text→再parse/check、保存Low、`@replace`の互換性も維持する。

self-hostingはコンパイラをNagiで書くこと、別backendはRust以外へ生成すること。この二つは別の候補で、今回の実装対象でも採用決定でもない。別backendの採用時には観測可能な契約を検査する。taskの全実行順、性能、アドレスの一致は求めない。

## 分離する後続タスク

既存SQLite/resource作業と同時に全面変更しない。各実装PRは契約を具体化し、最小のfailing testsから始める。このADRへの合意を、未決の構文や全既存コードの破壊への承認として扱わない。

### OWN-04: 既存所有値の代入

- before: 非Copyの`a = b`は暗黙move。after: 新値を作る式と既存値を渡す式を分け、後者の操作を明示する。
- 先に決める: 正確なCopy表、明示moveの構文、view/shared handle、field/match、引数・returnへの適用範囲。サイズ閾値で軽い値を判定しない。
- 移行: 対象版と既存例の書換えを提示し、High/Lowで同じ規則を採る。現行例は新規則の実装前に書き換えない。引数等へ一括拡大しない。
- 成功条件: Copy代入、新値生成、非Copyの暗黙代入の拒否、明示move後の使用拒否、再初期化、借用中move。診断の元位置、High/保存Low/手書きLow、生成Rust build/run、cleanup責任を検査する。

### ASYNC-03/04: 結果handleと失敗の分類

- before: spawnは文、子出力はunit/Result[unit, Error]、子Errでscope失敗。after: scopeが寿命と故障を保持し、handle経由でTまたはResult[U, E]をそのまま受け取る。業務Errだけでは兄弟を止めない。
- 先に決める: 一度限りの取得、handle型と故障/取消型、未受取Resultの扱い、複数故障、検出時点、scope外へ持ち出せるか。非Copy結果を二重取得させない。
- 移行: 現行Supervisor terminal Errをtask故障へつなぐ明示方法を設計し、HTTPとの共倒れ停止を黙って失わない。scope本体のtry退出と親Future Dropは別経路。古い契約の負例を無説明に緩めない。
- 成功条件: unit、非Copy T、ResultのOk/Err、未受取、非Copy結果の二重取得拒否、兄弟継続/取消、親body Err/panic/Drop、明示shutdown、non-yielding処理、SupervisorとHTTPの終了。Copy結果も含めた再awaitとhandleの消費規則は詳細設計へ残す。実runtimeのbarrierで先後関係を確認する。sleepだけで順序を決めない。

### ACTOR-01: 条件付きshared message

- before: message/replyのsharedは拒否。after: 条件を満たす明示sharedを許す方向。
- 先に決める: Send/Sync、内部可変性、容量課金、資源保持、replyへの流出。sharedという表示名だけの許可リストにしない。
- 移行: 既存move messageを維持し、native資源やviewを無条件に解禁しない。descriptorと用途別capability、Rust側のtrait検査を照合する。
- 成功条件: 許可payloadと拒否payloadを対にする。入れ子、alias、登録context、capacity解放、timeout後の仕事、sharedの循環や長寿命保持の限界を含む。High/Low/生成Rustと実actorを検査する。

## 今回採らないもの

- 将来方針を現在のリファレンスへ上書きすること。
- 任意Futureの保存解禁、全型の暗黙Copy、新しいtimeout/default、独自GC、async destructor。
- Low廃止、全面Typed IR/SSA化、新VM/backend、runtime交換。
- nullable/Result/panic、取消要求/終了、SQL Err/rollbackを同じ失敗として扱うこと。
- 仕様を複製した新しい`semantics.md`。現行契約は[language-invariants](../language-invariants.md)、内部経路は[compiler-pipeline](../compiler-pipeline.md)を参照する。

## 未決と参考

[Q-005〜007](../open-questions.md#q-005-既存所有値の代入を明示する範囲)へ細部を集約する。整数overflow、文字列index、class比較、Mapのkey、mutable globalは今回決めない。debug/release overflowの現行差は既存契約に残す。

比較の参考: [Pythonの代入](https://docs.python.org/3.14/reference/simple_stmts.html#assignment-statements)、[Python TaskGroup](https://docs.python.org/3.14/library/asyncio-task.html)、[Rust Arc](https://doc.rust-lang.org/std/sync/struct.Arc.html)、[Tokio JoinHandle](https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html)、[Elixir Supervisor](https://elixir.hexdocs.pm/Supervisor.html)。Pythonの参照代入やTaskGroup、RustのArc、Elixirの監視を説明の足場に使う。それらと同じ機能・取消・隔離を提供する根拠にはしない。
