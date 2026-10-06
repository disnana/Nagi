# 明示moveとtask結果の実装順

状態: 明示moveの意味論と非Copy既存値の通常代入移行は確定。作者は既存設計に沿うAPIの選択と実装を承認した。以下のmove仕様は採用済み・実装中・未リリース。taskの残る意味論は別工程で決める。

監査基点はmain `e7aff1da0a36503d239d70cf5dbcf892655978e0`（#84反映済み）。[ADR 011](adr/011-language-behavior-and-docs.md)の方向を実装へ移すための計画である。契約の正本は[language-invariants](language-invariants.md)、未決の管理は[Q-005/006](open-questions.md#q-005-既存所有値の代入を明示する範囲)に残す。

## 依存関係と順番

SQLiteの[段階計画](compiler-rust-boundary-plan.md)は別に継続する。#84の多接続・終了観測と[PR #85](https://github.com/disnana/Nagi/pull/85)の取得予算はprivate試作であり、新しい公開Pool/Txやtask handleではない。今回の言語仕様の実装依頼は、その計画のPhase 5やAuth Scopeの開始承認とは扱わない。

moveの追加は現在のmodule identity、checked facts、生成planを使う。SQLiteのprivate adapterに依存しない。task結果の設計にはmove後の二重利用検査を使うが、SQLiteの公開配線を同時に変更する必要はない。Tx等の転送禁止をtask handleの追加で緩めない。

| 順番 | 差分 | 互換性と次へ進む条件 |
|---|---|---|
| V1 | canonical明示moveを追加 | 仕様→失敗テスト→実装。入力の型・origin・async provenanceとcleanup責任を維持し、Rustは入力を一度だけ評価する |
| V2 | 既存非Copyローカルの単純代入に明示操作を求める | V1と同じdraft実装PRに順に積む。承認済みの受理変更としてサンプル・日英Docs・負例を移行し、全既存testと4 OS CIまで確認する |
| S1 | scope所属の一回限りの結果handleと、新経路の業務Result・故障境界 | handle・故障・未受取の契約を具体化してから、失敗テスト→小さい縦切り実装。Resultを受け取る新経路では、この段階からErrを値として扱う。旧statement spawnと第一級Future一般の解禁は別 |
| S2 | 旧statement spawnの移行とサービス故障の接続 | S1とSupervisorの移行例が揃ってから実装。現在のterminal Err→HTTP終了を消さない。新経路に業務Err分離がないまま「handle完成」としない |

各差分はdraft PRとし、CI結果を確認する。merge、版更新、releaseは別の判断。互換性を壊す変更を「文書の修正」や「内部整理」として混ぜない。

## 現行の根拠

| 項目 | 現行 | 実装・継続検査 |
|---|---|---|
| 非Copy代入 | `destination = source`がsourceを消費する。move専用の公開操作はない | [check.rs](../../compiler/src/check.rs)のAssign/consume、[ownership](../../compiler/tests/ownership.rs) |
| Copy | 下表の型・定義をcheckerが判定する。サイズで決めない | check.rsのcopy_type、[shared field](../../compiler/tests/shared_field_moves.rs)、[async値](../../compiler/tests/async_value_types.rs) |
| 明示copy | `copy(view(...))`で独立した値を作る。暗黙Copyとは別で、RustのClone実装を必要とする場合がある | [copy capabilities](../../compiler/tests/copy_capabilities.rs) |
| spawn | 文。子出力はunitかResult[unit, Error]。引数を親で評価し、所有値を子へ渡す | [parser](../../compiler/src/parser.rs)、check.rs、[scoped tasks](../../compiler/tests/scoped_tasks.rs) |
| scopeの失敗 | 本体終了後のjoinで子Err/panicを観測し、兄弟を止めて終了を待つ | [Scope](../../runtime/src/concurrent.rs)、[実Scope契約](../../compiler/tests/scope_runtime_contract.rs) |
| Supervisor | runのterminal Errが同scopeのHTTPを止める。replyの業務Errとは別 | [actor lifecycle](../../runtime/src/actor/lifecycle.rs)、[公開Supervisor](../supervisor.md) |

### Copy判定を同時に変更しない

V1/V2は、現行checkerのCopy判定を据え置く。primitive-onlyへの縮小や全enumのCopy化は行わない。

| 型 | 現行の暗黙Copy |
|---|---|
| 8整数幅、f32/f64、bool、unit、UUID、timestamp | 可 |
| view | 可。ただしorigin、借用先の有効期間、読み取り専用の制約は残る |
| 関数値 | 可。対応済みのローカルasync関数別名を含む。Futureの保存・引数化の許可とは別 |
| class | 全fieldが再帰的にCopyなら可 |
| enum | 全variantの全payloadが再帰的にCopyなら可。空payloadも含む |
| owned[T]、T? | Tの判定を引き継ぐ |
| 登録済み標準resource | canonical registryのcopy値に従う |
| str、bytes、List、shared、Error、Db、Html、Result | 不可。ResultはOk/ErrのpayloadがCopyでも不可 |

判定には既存の再帰深さ制約もある。これは読み取り時のavailable/borrow検査の代替ではない。Type::is_copyだけで新たなCopy表を実装しない。shared handleのmoveとclone_sharedによる所有者追加、payload copyも区別する。

## V1/V2の採用仕様

### 採用する書き方（実装中・未リリース）

```text
from std.ownership import move

def main():
    source = "Nagi"
    destination = move(source)
    print(destination)
    source = "new"
    print(source)
```

標準operationのidentityで解決し、qualified importとaliasも通常のmodule規則に従う。`move`を新しい予約語や常設builtinにしない。同じ名前のユーザー関数・変数を、綴りだけで所有権操作と判定しない。

moveは一度評価した入力をそのまま渡す。非Copy値なら元placeを消費し、Copy値なら既存のCopy規則が働く。clone、Arc所有者の追加、allocation、closeを行う操作ではない。引数・return・field等の既存consume検査を迂回できない。

引数は一つ、戻り型は入力から推論し、明示型引数は受け付けない。入力は既存consume規則で渡せる式で、ローカル変数だけに限定しない。fresh値やCopy値への指定は任意。Futureそのものや入れ子のFutureをmoveへ渡すこと、非Copy index取得、borrowed/shared fieldからの所有値取得は既存の拒否を保つ。awaitの結果値は対応する型なら渡せる。

V2で新たに拒否するのは、代入の右辺が解決済みローカル変数そのもので、その型がnonCopyの場合だけとする。括弧で包んだ同じ変数も同じ対象。宣言、型注釈付き代入、再代入を含む。`a = User(...)`や関数呼出し、try、field/index、引数、return、matchを一括で変更しない。この最初の範囲は「すべての所有権移動に明示moveを要求する」規則ではない。

viewの代入はCopyとして維持。sharedはnonCopyなので、V2の単純代入ならmoveかclone_sharedを選ぶ。`move(shared_value)`はpayloadの独立copyではない。

| 案 | 判断理由 |
|---|---|
| canonical `std.ownership.move` | 採用。既存のimport/aliasとchecked operationの経路を使い、未importの名前を占有しない |
| contextual `move source` | 不採用。専用ASTと全遍歴・Low印字の追加が必要。`move(...)`が既存関数呼出しの場合との説明も必要 |
| 常設builtin `move(...)` | 見送る案。ユーザー関数のshadowで同じ見た目の意味が変わりやすい |
| 普通のRust identity関数だけ | 不十分。checkerでconsume/originを確定し、明示操作のidentityを保持する責任が残る |
| Copyをprimitive-onlyに縮小 | 見送る案。代入移行と別の互換性変更を同時に増やす |

### 実装の境界

標準module/operationを追加するだけでは完了しない。[checked.rs](../../compiler/src/check/checked.rs)のoperation planは現在Rust呼出しを出力するため、identity transferの確定した生成actionを持たせる。emitterで`name == "move"`と再推論しない。新しいruntime helperを追加せず、封印したidentity生成を使う。実装・測定前にコスト0と断定しない。

viewを含む所有List/Option/Result等を移す場合は、入力のoriginと入れ子の位置をそのまま保つ。borrow_ownerは「借用を作るoperation」の契約なので、その値だけでidentity transferを代用しない。consume、origin、storage/cleanup planの三つを照合する。未対応originをstaticへ変えたりcloneで回避したりしない。

透過的な値転送をoperationの閉じた意味として定め、origin_at/content_origins、views_absent、borrows_temporaryへ同じ入力対応を使う。コピー可能なasync関数別名へ操作を使う場合も、async呼出しのprovenanceを失わない。新しい標準operationを普通の第一級関数値にする機能は今回追加しない。

### 先行テストと移行

tests-only差分で、operation未実装による失敗と、旧実装が暗黙代入を受理することを新しい拒否oracleが検出する失敗を別に記録する。今回の承認で変えるのは通常代入の期待値であり、use-after-moveや借用・Drop・評価順の既存oracleは弱めない。

- pass: Copy表の各代表、非Copyの明示move、新値生成、再初期化、sharedのmove/clone_shared、alias/qualified import、ユーザーの同名関数。
- identity: 引数数・明示型引数の不正、Low metadataの偽装、import aliasのshadowを拒否または通常の名前解決として処理し、誤ったoperationへ変えない。
- fail: move後の再利用、借用中のmove、shared親からの非Copy field取得、Result裸破棄、局所viewのescape。V2では裸のnonCopyローカル代入も拒否。
- conformance: High、生成Low、保存Low、独立した手書きLowをcheckし、生成Rustをbuild/runする。表示名とcanonical identity、元の拒否行、入れ子view、branch/loop/function valueを確認する。
- lifecycle: RHS Err/panic、旧値Drop、正常/取消時の破棄回数と順序を既存harnessで観測する。moveをclose/rollback完了と説明しない。
- cost: 同じプログラムの旧暗黙moveと新操作を比較する。追加call/clone/allocation、generated Future frame、compile時間を必要な範囲で測る。文字列一致だけで意味同値を保証しない。

移行時はサンプルを分類してから書き換える。普通の関数引数まで機械的にmoveで包まない。入門はPythonとの比較、動く短い例、出力、move後再利用の誤りと直し方を日英で揃える。移行前の実行記録を残し、実装・検証前の例を現行公開版で動く例と説明しない。

## S1/S2: task結果の境界

以下は詳細設計の候補で、使用可能な構文ではない。

```text
async with scope:
    task = spawn fetch_user()
    received = await task
```

taskがTを返す場合はT、Result[U, E]を返す場合はそのResultを受け取る。業務Errを故障へ自動昇格しない。故障を返却値の外側に置くなら、結果は概念的にResult[T, TaskFailure]となり、Result taskは二重Resultになる。これはactor callで使う「輸送失敗とreplyを分ける」規則とも比較する。

| 決める細部 | 初版の検討案・検証条件 |
|---|---|
| handleの取得 | 型名は候補Task[T]。awaitでconsumeし、Copy結果を含め二度目を拒否。implicit cloneは禁止 |
| 所有者 | 生存scopeだけがJoinSetとjoin記録を所有する。handle Dropでdetachしない。故障はscope退出・明示受取等、決めた検出点で観測する。本体中のbackground監視を既に提供するとはしない |
| 結果受取とjoin | oneshotの通知だけをtaskの終了確認としない。scopeによる実join、出力の保持・破棄、task Future Dropの順序を観測する |
| 受取待ちの取消 | awaitはhandleをconsumeし、そのPending Futureが破棄されても再取得しない案。子停止を自動的に意味しない。生存scopeは実join責任を持ち、scope自身のDropはabort要求まで |
| scope外持出し | 初版は拒否する案。alias、Option/Result、class/List、return、子taskへの転送で抜けないことを検査する。普通のRust 'staticだけに委譲しない |
| 未受取 | unit以外の省略、Resultを未使用変数へ入れる場合、branch/loopの未取得を仕様化する。現行の裸Result拒否だけで全未取得を検出できるとはしない |
| 故障 | panic、取消、明示的サービス故障を業務Errと分ける。型、複数故障、body Errと子faultのprimary/関連診断、検出時点をS1実装前に決める |
| 親の退出 | body Errは取消とjoin完了を待つ。親Future Drop/unwindは取消要求まで。受理済みDBや外部副作用は戻らない |
| Supervisor連携 | terminal Errを明示的なtask故障へ変換する経路を用意するか、旧fail-fast spawnを移行中に維持する。HTTPの永久待ちを回帰で検出する |

既存statement spawnをすぐ値取得経路へ置き換える案は採らない。旧経路を残して結果handleを追加する案と、全spawnの契約を切り替える案を比較し、削除時期・故障の昇格方法を決める。結果をoneshotで送るだけ、Tokio JoinHandleを捨てるだけ、独立observer taskを足すだけではscopeの終了責任を満たさない。

runtimeの最小縦切りでは、異種Tの結果チャネルとscope所有のjoinを分ける案を試す。独自GC、async destructor、general region/effect checker、任意Future保存を同時に導入しない。安全なscope依存のawaitが現在の生成構造で表現できるかを先に検証する。

現Scopeのshutdownはabort要求後に実joinをdrainするが、その途中の後続Err/panicは返さない。複数faultや取消競合のcauseを保持する新契約なら、abort_allとjoin_next_with_idによる手動drainを比較する。旧body Err→cancel→元Err伝播のprimaryを、子の後続panicで無説明に上書きしない。

正常/業務Ok/業務Err、panic、handle二重取得、未受取、兄弟継続と取消、body Err/panic/Drop、明示shutdown、non-yielding、SupervisorとHTTP停止をbarrierで観測する。結果送信、取消要求、Future破棄、join完了を別のeventとしてassertする。cancel/closeの名前だけで完了を判定しない。

既存のscoped_tasksには評価順用のruntime stubもある。それをTokioの取消・join完了の実証と数えない。scope_runtime_contractは実runtimeを使う。supervised-serviceの検査は業務409・正常shutdown等を含むが、Supervisor terminal故障からHTTP取消への専用統合oracleではない。S2ではその経路を実socketとbarrierで追加する。

## 移行前の監査と実装の進行条件

基点mainで関連する既存7 suite・36件を実行し、成功した。内訳はownership 14、shared_field_moves 4、copy_capabilities 2、result_discard 4、async_value_types 6、scoped_tasks 5、scope_runtime_contract 1。これは現在の契約の検査で、候補moveやTaskの実装成功ではない。

実行コマンドは`cargo test --locked -p nagic --test ownership --test shared_field_moves --test copy_capabilities --test result_discard --test async_value_types --test scoped_tasks --test scope_runtime_contract`。Linux、既存debug/test profile、offline依存cacheで実行した。source treeは`117533295a2bbd16d12d413cb00f72076d17133e`、raw log SHA-256は`e8bf0664eb1287ef05f38fb4896ce6bce6c416ca10124625d2b15ff74b4b500f`。

さらに[9例の移行前検証](value-task-audit-results.md)を実行した。High/手書きLow18入力と生成保存Low6入力のcheckは受理18・期待した拒否6。正常6例をHigh/保存Low/手書きLowでbuild/runし、18実行の出力が一致した。既存Copy class/enum・nullable・view、async関数別名、ユーザーのmove識別子、Result/sharedのnonCopy、借用中move、所有view containerの移動を確認した。新APIの実装・コスト検証には数えない。

V1/V2の意味論は確定済みで、既存設計に沿う具体APIの選択も今回の指示で認められた。同じ大枠を再質問せず、仕様→先行test→High/Low check→生成→Docs移行→全回帰→4 OS CIの順で進める。#85/#86はdraftのまま保ち、実装は新しいdraft PRへ分離する。moveの完了後はS1、業務Errと故障の分離、公開SQLite Pool/Txの順に進める。意味論への大きな未決だけ具体案をまとめて確認し、merge/releaseは実行しない。
