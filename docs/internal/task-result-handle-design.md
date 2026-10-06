# S1: scope所属のtask結果handle

状態: **詳細意味論を設計採用・未実装**。[ADR 012](adr/012-task-result-handles.md)で全Tの正常出口await/discardとscope故障のsticky保持を採用した。今回の自律判断の委任による新しい詳細判断であり、過去の承認から必然だったとは扱わない。名称・構文・runtime接続は以下の推奨案で、現行parser・checker・runtimeや公開版を変更していない。実装開始、CI、main反映、releaseとは区別する。

[ADR 011](adr/011-language-behavior-and-docs.md#async-0304-結果handleと失敗の分類)、[実装順S1/S2](value-task-implementation-plan.md#s1s2-task結果の境界)、[Q-006](open-questions.md#q-006-spawn結果handleと業務errtask故障)を前提とする。scopeが寿命と実joinを所有し、普通のResultの業務Errをtask故障へ昇格しない方向は既に採用されている。scope-local・全Tの一回受取と正常出口await/discard、業務Resultの外側に置く故障、sticky faultは採用する初版意味論である。Task/TaskFailure等のAPI名と具体構文は接続候補で、使用可能な標準機能ではない。[独立レビュー](task-result-handle-review.md)に選択理由と不採用案をまとめる。

## 現行との差と読取根拠

読取基点は作業branchの`d74e9651d6272c1390f411c2870afe30950f1c7f`。move作業中の基点であり、S1の実装commitではない。先行監査`/tmp/nagi-spawn-audit.md`はmain `e7aff1d`の記録で、今回も以下の経路を読み直した。今回Cargoやruntime試験は実行していない。追加レビューの読取基点は#87 head `e4089735de35e5f8496a1191b870e7b5f7edffc9`であり、以下の改訂も設計用branchだけの差分である。

| 境界 | 現行の事実 | S1の候補 |
|---|---|---|
| spawn | [AST](../../compiler/src/ast.rs)の`S::Spawn(Expr)`という文。[parser](../../compiler/src/parser.rs)は結果bindingを持たない | scope内の結果binding文を追加し、旧statement spawnも保持 |
| 子出力 | [checker](../../compiler/src/check.rs)はFuture[unit]かFuture[Result[unit, Error]]のみ。借用入力と未対応Future保存は拒否 | 対応する所有Tの結果handle。Result[U, E]はTのまま |
| scope | [runtime Scope](../../runtime/src/concurrent.rs)はJoinSet[Result[unit, Error]]の唯一owner。joinで子Err/panicを観測し、shutdownで兄弟を取消・drain | 異種Tの結果と均一なjoin記録を分け、受取も同じownerがjoinを進める |
| 親の退出 | [生成](../../compiler/src/emit.rs)は同一coroutineのlabelへbody Errを出し、body localsの退役後にcancelをawait。正常出口でjoin | label、元のcleanup anchorとError変換を維持。新たなbody asyncを作らない |
| 検出点 | 子の故障はbody終了後のjoinで観測。本体へbackground割込みしない | 明示受取中にもjoinを観測する。それ以外のbodyへ割り込まない |
| 既存試験 | [scoped_tasks](../../compiler/tests/scoped_tasks.rs)は一部runtime stubで評価順を検査。[scope_runtime_contract](../../compiler/tests/scope_runtime_contract.rs)は実runtimeでHigh/保存Low/手書きLowの退出と取消を検査 | 既存oracleを保持し、結果・actual join・業務Err・terminal連携の独立oracleを追加 |
| Supervisor | [actor run](../../runtime/src/actor.rs)と[terminal待ち](../../runtime/src/actor/lifecycle.rs)はcontext cleanup・実join後にterminalを返す。[service例](../../test-nagi-code/library-examples/supervised-service/main.nagi)は旧spawnのmonitor ErrでHTTP兄弟を止める | 旧経路を残す。Resultを値にするだけの置換で停止連携を失わない |

依存はCargo.lockのTokio 1.53.1。stock [join_next_with_id](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html#method.join_next_with_id)は取消安全で、成功時のIDとJoinError::idで終了taskを特定できる。[abort_all](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html#method.abort_all)はtaskをsetから除去せず、完了にはdrainが必要。[shutdown](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html#method.shutdown)は後続panicを無視するため、新しい故障記録を残す経路では手動drainを比較する。

Tokioの[task ID](https://docs.rs/tokio/1.53.1/tokio/task/struct.Id.html)は、task終了とactive JoinHandle/AbortHandle/JoinSet等の所有終了後に再利用できる。実join後も未受取recordを残す本案ではnative IDを永続entry keyにしない。

依存sourceの`tests/task_join_set.rs`はabort_on_drop、abort_all、runtime_gone、task_panics、ID付きjoinを扱う。読取のみで、この依存試験を今回実行したとは数えない。sender消失の観測はNagiのactual join契約そのものではない。

## 推奨する言語の入口

canonical moduleは候補`std.task`、handleは`Task[T]`、外側の故障は`TaskFailure`とする。一般のFuture値と区別し、`await handle`で`Result[T, TaskFailure]`を一回受け取る。TがResult[U, E]なら結果はResult[Result[U, E], TaskFailure]であり、flattenしない。

次は**未実装のHigh候補**。内側Errは業務結果として処理され、scopeを故障状態にしない。子がpanicした場合は外側Errとなり、下記の採用したsticky故障方針ではmatchで表示してもscope出口は失敗する。

```text
import std.task as tasklib

async def calculate() -> Result[i64, Error]:
    return error("ordinary rejection")

async def main() -> Result[unit, Error]:
    async with scope:
        task: tasklib.Task[Result[i64, Error]] = spawn calculate()
        match await task:
            case Ok(outcome):
                match outcome:
                    case Ok(value):
                        print(value)
                    case Err(_):
                        print("business error")
            case Err(_):
                print("task failure")
    return ok(print("done"))
```

業務Errだけなら期待出力は`business error`、`done`。同じ構造のLow候補は次の通りで、現行Lowの受理例ではない。

```text
import std.task as tasklib;
async fn calculate() -> Result[i64, Error] {
    return error("ordinary rejection");
}
async fn main() -> Result[unit, Error] {
    scope {
        let task: tasklib.Task[Result[i64, Error]] = spawn calculate();
        match await task {
            case Ok(outcome) {
                match outcome {
                    case Ok(value) { print(value); }
                    case Err(_) { print("business error"); }
                }
            }
            case Err(_) { print("task failure"); }
        }
    }
    return ok(print("done"));
}
```

結果bindingは`name = spawn async_call(...)`という専用文に限定する。任意の式位置のspawn、Futureの保存、capturing closureを同時に追加しない。既存の`spawn work()`は文のままunit/Result[unit, Error]とfail-on-Errを維持する。結果bindingではunitもTとして受け取れる。

新しい予約語を増やさない。代入右辺の`spawn call(...)`を文脈で認識し、既存ユーザー関数の`value = spawn(42)`は普通のcallとして維持する。High/Lowで同じ区別を固定し、生成Lowに同じbinding文とcanonical type identityを残す。括弧付きの一般spawn式を初版へ広げない。

親側で引数を左から右へ評価し、従来のconsumeに従ってFutureを作り、その後scopeへ登録する。引数の評価だけを子へ遅延しない。既存のview/native borrow拒否、最終RustのSend + 'static検査を保つ。Tの実payloadには借用view、Future、Taskや転送禁止resourceを許可しない。関数署名に現れるviewを実payloadと混同せず、現在の関数値対応範囲も別に検査する。

## scope localと一回受取

| 項目 | 採用する初版意味論と接続案 |
|---|---|
| Copy/clone/share | TaskはTがCopyでも非Copy・非Clone・非shared。独立copy、JSON、actor message、field保存も拒否 |
| local alias | 同じscopeの`alias = move(task)`は許す。元bindingは消費済み。scope originと未受取義務をaliasへ移す |
| 受取 | `await task`がhandleをconsume。Copy結果でも二回目を拒否。結果T自体は従来の所有権規則で利用 |
| 未poll/取消 | 受取Futureを作った時点でhandleは消費済み。未pollやPending中にFutureをDropしてもhandleは復活しない。子のjoin責任はscopeに残る |
| 利用範囲 | 初版は作成時のscopeと同じscopeのlocalだけ。関数引数/return、class/enum/List/Option/Result/owned wrapperへの格納、他task転送を拒否。一般region/effect検査は導入しない |
| nested scope | 外scopeのTaskを内scopeで受け取る/移すことは初版で拒否。内scope終了後に外scopeで受け取れる。生成変数のshadowでownerを取り違えない |
| 退出 | 全Tの正常経路でawaitまたは明示discardを必須とする。body Err/panic/親取消による異常退出はscope cleanupへ渡す |

canonical `std.task.discard(handle)`を候補にする。同期的にhandleをconsumeして受取を放棄するが、子をdetach/停止せず、join完了を返さない。Task[Result[U, E]]も明示discardは許す。これは業務Errの自動故障化ではない。unitを含めて全handleに同じ規則を求める初版方針を採用し、結果不要の旧statement spawnとの区別を読める形にする。

採用する全Tの正常出口await/discard義務を、checkerは単純なpending handle義務をscope/binding IDで記録する。moveは義務の移動、await/discardは解除。分岐の継続経路はすべて解除されていること、loop内bindingはbackedge/正常exit前に解除されていることを検査する。終了経路を継続joinへ混ぜない。例外的退出を「正常に未受取」として拒否しない。現行の未使用Result変数検査や通常のuse-after-moveだけでmust-consumeを実装済みとは扱わない。

awaitで得たTがResultの場合、その後のResult扱いは現行の裸式拒否・try/match規則を維持する。`result = await task`のbindingだけで業務Errを必ず処理したことにはならない。一般の未使用Result検出まで同時に変更しない。

## 唯一ownerのruntime比較案

最初は私有prototypeで次の構造を比較する。これは実装コードではない。

```text
Scope:
    JoinSet<ChildExit>             # 全taskのjoinを唯一所有
    entry[scope_ticket]            # mode、実join結果、handle状態
    unjoined[native_task_id] -> scope_ticket  # 未join taskだけの対応
    primary_fault / related_faults

Task<T>:
    scope_identity / ticket
    oneshot::Receiver<T>           # typed output。JoinHandleは持たない
    receipt_state                 # 受取/放棄の小さい状態。Scope/JoinSetへの強参照なし

spawn_value(Future<Output=T>) -> Task<T>
receive(&mut Scope, Task<T>) -> Future<Output=Result<T, TaskFailure>>
```

scope内ticketはscopeの生存期間に再利用せず、entryの唯一keyとする。native ID→ticket対応はspawn登録時に作り、実join時に対応を解決してentryへ記録し、未join対応を除く。join後にnative IDが新taskへ再利用されても、古い未受取のtyped receiverやrecordへ結び付けない。native IDは安定した外部IDにも使わない。

Task[T]のTは遅延して受け取る実payloadであり、nominal phantomではない。resource descriptorの保持関係、nonCopy/nonClone/nonshared、field/Serde/actor保存拒否を独立inventoryで確認する。用途別capabilityとcallback署名を同じ遍歴へ潰さない。既存resourceや公開OperationInfoの形を、この追加のために一括変更しない。

異種Tはtyped oneshotへ渡し、JoinSetへは均一な終了modeだけを返す。Any/downcast、unsafe、追加join observer task、scopeへのArc強参照をhandleへ追加する案は採らない。receiptは受取/放棄を表す小さい共有状態までで、ScopeやJoinSetへの強Arc参照・owner callbackを持たない。必要なchannel/entryのコストは測る。alloc-freeやFutureサイズ不変は約束しない。

採用したsticky故障を満たすreceiveの接続案は次の通り。実装・取消時の成立は先行prototypeで検証する。

1. owner identityを照合する。生存scopeだけが`&mut`でjoinをpollし、同scopeの並行receive ownerを作らない。手書きRustの誤ownerでも他scopeのjoinを奪わない。
2. 既に観測したfaultを先に確認する。無ければtargetの実join済み記録を調べる。まだなら`join_next_with_id`を進める。
3. target以外の終了もnative IDからticketを解決してentryへ記録する。join Readyによるsetからの除去→actual join記録→未join対応の除去→cause/取消状態の更新には別のawaitを挟まない。任意TのDropやユーザーcallbackをこの記録区間へ入れない。join_next_with_idのPending中の取消安全性と、Ready後のrecord保持は別に検査する。生存scopeでreceive Futureを取消しても記録を失わない。
4. targetのactual join成功後にtyped outputを取り出す。sender Ready、sender Drop、業務ResultのReadyを終了証拠にしない。成功join後に生存receiverへ値が無い場合はprotocol故障であり、来ない値を永久に待たない。
5. faultを観測したならprimaryを固定し、abort要求後に残る実joinをdrainする。typed outputを成功として返さず、drain完了後に外側Errを返す。

別handleのjoin成功を先に観測しても、Tはそのtyped receiverに保持する。後の受取が可能でなければDropで解放する。explicit discard、取消されたreceive、join済みhandleの退役をentry側で識別し、全履歴のrecordをscope中ずっと保存しない。保持するrecordは未join/未受取/故障診断の責任に必要なものだけ。background joinを置かないため、長いbody中では終了済みでも未joinのtaskがJoinSet/entryへ残り、join済みでも未受取Tはchannelへ残る。メモリ量を実行中task数だけに限定しない。実join済みかつ受取/放棄済みのentryは退役できる。scope退出で残りを退役する。

receiverを先に破棄すると、送信失敗TのDropはchild側、送信済みbuffer TのDropはparent側で起き得る。任意Rust TのDrop panicはそれぞれの実行経路で分類し、内部lockや終了記録区間へ任意Dropを持ち込まない。discardによる放棄は子の停止、実join、任意結果資源のclose成功を確認する操作ではない。Drop panic、二重unwind、abortからの普遍的回復も保証しない。結果Tを受取後も保持している場合、子のjoin成功はTのclose/Drop完了を意味しない。

受取Future Dropはtyped receiverの放棄とowner借用の解除まで。生存scopeのJoinSetと終了記録を保持する。join/drain待ちをDropした場合も次のscope操作から再開できる。scope自体のDropはabort要求だけで、完了記録やjoin ownerが別observerへ永続移譲される保証を足さない。

## 業務値、故障、primary

初版は**故障をscopeへstickyに保持する**。受取Err処理後もscope出口を失敗にするのは今回採用する新しい回復性の契約で、現Scopeの変更済み挙動ではない。現Scopeはsticky fieldを持たず、joinが故障を返した後の再joinは空setでOkになり得るが、現Nagi生成はこの回復経路を公開しない。以下のprimary/退出表と故障後bodyは、実装が満たす採用仕様である。普通の子Result Errだけではsticky状態にせず、健康な兄弟は継続する。一方、panic、要求していない取消、旧statement childのErr、内部protocol故障はscope故障とし、最初の観測で兄弟へabortを要求する。TaskFailureは候補のopaque標準型で、kindはPanicked / Cancelled / LegacyError / Internal、診断は読み取り専用とする。canonical kind/message操作で読む形を推奨し、messageのviewはfailureを借用元にする。legacy child Errは元Errorをscope出口へ保ち、任意のnative panic payloadをNagiの業務Eへ変換しない。

受取で別の子の故障を先に観測した場合もscope全体のprimaryを返す。対象task自身が成功していても、観測済みscope故障があれば成功へ戻さない。未観測の兄弟故障をtarget成功前に必ず検出する保証はない。どの故障が先に観測されるかはscheduler順であり、spawn順ではない。

受取Errをmatchして診断してもscopeは成功へ戻らない。これは回復可能な業務Errとは違う。bodyへ強制割込みはしない。故障後にbodyがさらにspawnする場合も親の入力評価を維持し、実taskを登録して直ちにabortを要求する。fake join済みhandleは作らず、そのtaskも後のreceive/出口で実joinする。fault後の新登録でscopeを健康状態へ戻さない。non-yielding taskや止まらないbodyを時刻で強制停止する機能はない。

| 退出/競合 | primaryと完了条件 |
|---|---|
| 子T、Result Ok/Err | actual join成功後にOk(T)。業務ErrはTの内側。scope failureは作らない |
| 子faultを受取/出口で観測 | 最初に観測したfaultをprimaryとして保持。兄弟abort→全実join。後続の実faultは関連記録へ残す |
| abort要求に伴うCancelled | 要求済みの取消はcleanup結果であり、新しいprimaryにしない。panic/旧child Errが取消と競合して実際に返った場合は故障として保持 |
| bodyのtry Err | 元のbody Eをprimaryとして返す。body locals退役→子abort/drain。drain中の子faultで元Eを置換しない |
| body panic/native Drop panic | unwindを隠さず、scope Dropでabort要求。同期Dropでjoin完了を返さない。二重panic/abort/OOMからの普遍的回復は保証しない |
| 親Future Drop/未poll取消 | 将来のbody結果を作らない。scopeが構築済みならDropでabort要求。子の完了は別のruntime観測で確認 |
| receive/drain Future Drop、生存scope | handleは消費済み、join責任とprimaryはscopeに残る。再開時にrecordを再利用し、同じ結果を二重取得しない |

body Errの後続faultはcleanupの内部関連記録として検査する。既存Error/class/enumへ汎用の診断channelを新設したとは扱わず、公開Eへ無理に詰め込まない。通常の子fault出口は従来scopeのError経路へ明示変換する候補とし、既存のRust `From<Error>`条件も維持する。受取時のTaskFailureとこのscope出口のErrorを同じ型と説明しない。

`try await task`は型上は外側TaskFailureの伝播であり、内側の業務Eを伝播しない。S1では現行のscope関数のError/From<Error>条件を維持するため、Error返却関数がこの式をそのまま使うことはできない。上の例のようにmatchを基本にし、TaskFailureをErrorへ変換する標準operationを接続する場合も名前付き・明示変換に留める。一般の暗黙エラー変換やscope返却エラー型の解禁は追加しない。

故障診断に保持するIDはscope内の対応付け用であり、安定した外部IDや故障の全順序ではない。関連faultを保存する場合も完了済みtask/Future/結果Tを診断のために保持し続けない。JoinErrorのnative panic payload破棄は任意Rustの境界で、内部lockや二重unwind中に安全に壊せるという保証を追加しない。

## SupervisorとHTTPの移行

S1では旧statement spawnをそのまま残す。[supervised-service](../../test-nagi-code/library-examples/supervised-service/main.nagi)の`spawn monitor(group)`と`spawn serve_web(...)`は変更しない。既存のterminal Err→scope故障→HTTP取消を維持する。新handleの`Task[Result[unit, Error]]`へ機械的に置き換えるとterminal Errが業務値になり、HTTPの終了待ちを継続する反例がある。

actor自身のreply業務Errはstateを保ち再起動しない。worker外層Err/panicとrestart policy、最後のworker/強度超過/明示停止、context破棄を経たterminalは別である。新TaskFailureへactor内部の分類を一括統合しない。

S2では明示service fault昇格か、monitorのResultを親が受けてbody Errへ進む移行を別に設計する。一般Result Errの自動昇格は戻さない。contextの外部Arc保持とSupervisor終了待ちの循環、既開始DB操作等の副作用もhandleで解決すると説明しない。S1 acceptanceには旧terminal連携の専用実socket回帰を含める。

## 先行validationと接続順

| oracle | positive/negativeと必要な観測 |
|---|---|
| 異種結果 | 同scopeでunit、Copy整数、非Copy文字列/class、Result Ok/Err。送受取の値・Drop数一致、業務Err後に健康兄弟がbarrierへ到達 |
| ID再利用 | private seamでAを偽native ID Nとして実join記録し、Aの結果を未受取で残す。新ticket Bへ同じNを割当ててjoinする。A/B各receiver・recordが混ざらず、一回ずつ元の値を受け取れる。Tokioの実ID再利用やsleepを待たない |
| publicationとjoin | sender Readyを確認してもchild cleanupのgate保持中はreceive Pending。gate解放→Future Drop→実join→受取。receiver通知だけでGREENにしない |
| 途中取消 | receive未poll/Pending Drop、target以外join記録済み、fault drain途中Drop。生存scopeの次操作でdrain・pending0、結果の二重受取なし |
| 未受取（採用仕様の先行oracle） | 正常branch/loop/scope出口のhandle放置をchecker拒否。await/明示discardは受理。unit/Copy/Resultを対にする。body Err/panic/親Dropはcleanup経路 |
| originとescape | move alias正常、二回await/copy/share拒否、nested scope/関数/field/container/別taskへの持出し拒否。Tの実view payloadと関数署名viewを区別 |
| 生成境界 | 親の引数左→右、所有値move、borrow拒否、RHS Err/panic、元のview cleanup anchor、short-circuit、async aliasを既存oracleと併記 |
| 故障 | target/別子panic、予期しない取消、旧child Err、取消要求と実fault競合。primary固定、後続fault記録、全join後に受取Err |
| 親退出 | body Errが子faultを上書きされない。親panic/Drop時はabort要求と別のchild終了barrierを分ける。Drop直後のpending0を要求しない |
| 故障後body（採用仕様の先行oracle） | 受取Errをmatchしてもscope出口Err。次spawnは親引数評価→登録→abort→実joinとなり、scope成功へ戻らない |
| terminal連携 | TEMPORARY最後のworker故障・強度超過→実HTTP listener停止/handler cleanup。actor reply業務ErrならHTTP継続。正常shutdownも別case |
| Low/registry | High→保存Low→別check、独立手書きLow、canonical Task identity、fake metadata拒否、user spawn/move名を維持。受理はRust build/runまで |
| 任意TのDrop | receiver先放棄によるchild側送信失敗Drop、送信済みbufferのparent側Drop、成功受取後のT Dropを別に観測。可能な単一panic分類と二重panic/abortの限界を分け、discard/実joinをclose完了と数えない |
| コスト/制約 | oneshot/entry確保、長いbodyの完了未join task、未受取T、Future frame、大量taskのretireを同条件で測る。新capacity/timeoutを追加せず、non-yielding/native blockingの限界を残す |

順序はoneshot、Notify、独立Atomic event、native Drop gate等で観測する。watchdogの時間はhang検出だけに使う。watchdog失敗でもgate解放→cancel/drain→fixture closeを行ってからassertし、cleanup前のdirectory削除や二重panicを避ける。Sender ready、取消要求、Future破棄、actual join、結果受取を別eventとしてassertする。

1. private runtimeのtests-only→compile RED→小実装。publication前倒しとoneshot-only版のruntime REDも保存する。公開language未実装の状態でscope.receiveの成立を確かめる。
2. High/Lowの独立正常・異常oracleを先に固定し、resource registryとscope/handle義務のchecked factsを追加する。canonical IDとscope IDを分け、type名のwhitelistで推論しない。
3. sealed Await/Spawn/Discard planから実ownerへ生成する。scope label/既存Error変換/cleanup位置を維持し、初版のscope局所性を最終Rust任せにしない。
4. 既存statement spawn/actor/HTTP回帰、元位置、native実行、4 OSを揃える。runtime prototype成功と公開language完成、merge/releaseを分けて記録する。

## 二つの詳細判断を採用

以下二点は既承認の方向から一意に導けるものではないが、追加の「安全に判断できるものは理由を示して自律確定」という委任に基づき、[ADR 012](adr/012-task-result-handles.md#根拠と今回の判断)でA+Aを採用した。過去の確認待ちだけを理由に停止を続けない。名称、owner bookkeeping、High/Lowの接続は上の推奨案を小さいprototypeで検証する。

| 採用判断 | 理由 | 不採用案との差 |
|---|---|---|
| 全Tの正常出口await/discard | 新Task結果bindingの受取/放棄意図を明示し、返却型変更で義務を消さない。異常退出はcleanup。一般owned/Result bindingへ義務を広げない | 暗黙Drop許可は小実装だが未受取Resultを黙って捨てられる。Resultだけ義務化は型で規則が変わり、型変更で検査が消える。全面must-use Result/effect検査は別の言語変更 |
| sticky故障 | receiveで早く観測した故障を正常出口で消さず、既に兄弟abortしたscopeを表示だけで健康に戻さない。受取Err処理後もscope出口Err | 回復可能な単一子faultは兄弟継続に向くが、未受取fault/旧legacy Err/故障ackの意味を別に設計する必要がある。全spawnを同時に切替える案はSupervisor→HTTP移行をS1へ巻き込む |

設計候補`9ea6995`では二点を確認待ちとした。今回の委任によって詳細意味論を採用した履歴を残し、S1実装済み・検証成功とは扱わない。現行Scope子Err契約とTask未対応を維持し、実装順はS1→S2の明示service fault移行→公開Pool/Tx。[巨大capacityの公開前ブロッカー](sqlite-capacity-decision.md)は未解決のままである。


## Stage 1の実装記録

[先行REDとprivate bridge検証](task-bridge-stage1-results.md)はADR 012採用後の記録。cfg(test)試作で結果通知とactual join、receiptとsticky scope故障、元Errorの保持を検査した。新Task checker/生成、公的なTask API、High→保存Low→nativeは未実装。上の設計基点や未検証だった当時の記述を、成功済み保証へ読み替えない。
