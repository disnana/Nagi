# 明示moveとtask結果の実装順

状態: 明示moveの意味論と非Copy既存値の通常代入移行は確定。作者は既存設計に沿うAPIの選択と実装を承認した。以下のmove仕様は採用済みで、作業branch `feat/explicit-move-contract`に実装済み・未リリース。検証状況とmain反映は[実装結果](explicit-move-results.md)と[進捗](progress.md)で別に記録する。S1は[ADR 012](adr/012-task-result-handles.md)と[接続判断](task-handle-implementation.md)に沿い作業branchへ接続済み・未リリース。[接続結果](task-handles-s1-results.md)に実証範囲を記録する。以下の先行手順・未実装記述は設計時点の記録。

監査基点はmain `e7aff1da0a36503d239d70cf5dbcf892655978e0`（#84反映済み）。[ADR 011](adr/011-language-behavior-and-docs.md)の方向を実装へ移すための計画である。契約の正本は[language-invariants](language-invariants.md)、未決の管理は[Q-005/006](open-questions.md#q-005-既存所有値の代入を明示する範囲)に残す。

今回の最新参照はmain `7d2d96a`（#85/#86反映済み）とdraft #87 head `5985e1b`。この設計branchには#87のDocs・先行回帰・private SQLite close修正を取り込み、compiler/runtime/testsは#87と同bytesに揃えた。差分はS1詳細設計の11 Markdownだけ。S1の新構文・新Task・fault ledgerはまだ実装していない。#87の最終CI判定やmain反映とは分ける。

## 依存関係と順番

SQLiteの[段階計画](compiler-rust-boundary-plan.md)は別に継続する。#84の多接続・終了観測と[PR #85](https://github.com/disnana/Nagi/pull/85)の取得予算はprivate試作であり、新しい公開Pool/Txやtask handleではない。今回の言語仕様の実装依頼は、その計画のPhase 5やAuth Scopeの開始承認とは扱わない。

moveの追加は現在のmodule identity、checked facts、生成planを使う。SQLiteのprivate adapterに依存しない。task結果の設計にはmove後の二重利用検査を使うが、SQLiteの公開配線を同時に変更する必要はない。Tx等の転送禁止をtask handleの追加で緩めない。

| 順番 | 差分 | 互換性と次へ進む条件 |
|---|---|---|
| V1 | canonical明示moveを追加 | 仕様→失敗テスト→実装。入力の型・origin・async provenanceとcleanup責任を維持し、Rust標準identityへ入力を一度だけ値として渡す |
| V2 | 既存非Copyローカルの単純代入に明示操作を求める | V1と同じdraft実装PRに順に積む。承認済みの受理変更としてサンプル・日英Docs・負例を移行し、全既存testと4 OS CIまで確認する |
| S1 | scope所属の一回限りの結果handleと、新経路の業務Result・故障境界 | ADR 012の採用契約を先行失敗テスト→私有bridge→High/Lowの小さい縦切りへ接続。新経路は業務Resultと外側faultを分離。旧statement spawnを維持し、第一級Future一般の解禁は含めない |
| S2 | 旧statement spawnの移行とサービス故障の接続 | S1とSupervisorの移行例が揃ってから実装。現在のterminal Err→HTTP終了を消さない。新経路に業務Err分離がないまま「handle完成」としない |

実装差分は別PRとし、CI結果を確認する。#85/#86はユーザーがmainへ反映済み、moveの#87はdraftを維持する。merge、版更新、releaseは別の判断。互換性を壊す変更を「文書の修正」や「内部整理」として混ぜない。

## 移行前の基点と現行の根拠

次表の非Copy代入は監査基点main `e7aff1d`の動作である。V1/V2実装後の作業branchでは、所有する非Copyローカルそのものの代入を拒否し、`std.ownership.move`を使う。Copyと引数・return・field/index等の既存consume、spawn/Scopeは変更しない。

| 項目 | 移行前の基点 | 実装・継続検査 |
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

### 採用する書き方（作業branchで実装済み・未リリース）

```nagi
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

V2で新たに拒否するのは、代入の右辺が解決済みの所有ローカル変数（`NameResolution::Local`）そのもので、その型がnonCopyの場合だけである。borrowed loop localは従来の読み取り専用制約で拒否する。括弧で包んだ同じ変数も同じ対象。宣言、型注釈付き代入、再代入を含む。`a = User(...)`や関数呼出し、try、field/index、引数、return、matchを一括で変更しない。この最初の範囲は「すべての所有権移動に明示moveを要求する」規則ではない。

viewの代入はCopyとして維持。sharedはnonCopyなので、V2の単純代入ならmoveかclone_sharedを選ぶ。`move(shared_value)`はpayloadの独立copyではない。

| 案 | 判断理由 |
|---|---|
| canonical `std.ownership.move` | 採用。既存のimport/aliasとchecked operationの経路を使い、未importの名前を占有しない |
| contextual `move source` | 不採用。専用ASTと全遍歴・Low印字の追加が必要。`move(...)`が既存関数呼出しの場合との説明も必要 |
| 常設builtin `move(...)` | 見送る案。ユーザー関数のshadowで同じ見た目の意味が変わりやすい |
| 普通のRust identity関数だけ | 不十分。checkerでconsume/originを確定し、明示操作のidentityを保持する責任が残る |
| Copyをprimitive-onlyに縮小 | 見送る案。代入移行と別の互換性変更を同時に増やす |

### 実装の境界

標準module/operationを追加するだけでは完了しない。[checked.rs](../../compiler/src/check/checked.rs)の私有operation planはnative callとidentity transferを分ける。canonical operationの閉じたdescriptorから入力対応と生成actionを確定し、emitterで`name == "move"`と再推論しない。入力を一度だけ出力するidentity生成で、新しいruntime helperを追加しない。公開`OperationInfo`のfieldsは変更していない。全プログラムについてコスト0と断定しない。

viewを含む所有List/Option/Result等を移す場合は、入力のoriginと入れ子の位置をそのまま保つ。borrow_ownerは「借用を作るoperation」の契約なので、その値だけでidentity transferを代用しない。consume、origin、storage/cleanup planの三つを照合する。未対応originをstaticへ変えたりcloneで回避したりしない。

透過的な値転送をoperationの閉じた意味として定め、origin_at/content_origins、views_absent、borrows_temporaryへ同じ入力対応を使う。WholeValue転送は共通の型付き定数モデルにも入力の評価結果を透過し、moveで包んだ定数0等を検査する。Result map_errorのsuccess payload関係や、任意の関数呼出しの評価とは混同しない。コピー可能なasync関数別名へ操作を使う場合も、async呼出しのprovenanceを失わない。新しい標準operationを普通の第一級関数値にする機能は今回追加しない。

### 先行テストと移行

tests-only差分で、operation未実装による失敗と、旧実装が暗黙代入を受理することを新しい拒否oracleが検出する失敗を別に記録した。実装後の成功と継続検査は[進捗](progress.md)へ分けて記録する。今回の承認で変えるのは通常代入の期待値であり、use-after-moveや借用・Drop・評価順の既存oracleは弱めない。

- pass: Copy表の各代表、非Copyの明示move、新値生成、再初期化、sharedのmove/clone_shared、alias/qualified import、ユーザーの同名関数。
- identity: 引数数・明示型引数の不正、Low metadataの偽装、import aliasのshadowを拒否または通常の名前解決として処理し、誤ったoperationへ変えない。
- fail: move後の再利用、借用中のmove、shared親からの非Copy field取得、Result裸破棄、局所viewのescape。V2では裸のnonCopyローカル代入も拒否。
- conformance: High、生成Low、保存Low、独立した手書きLowをcheckし、生成Rustをbuild/runする。表示名とcanonical identity、元の拒否行、入れ子view、branch/loop/function valueを確認する。
- lifecycle: RHS Err/panic、旧値Drop、正常/取消時の破棄回数と順序を既存harnessで観測する。moveをclose/rollback完了と説明しない。
- cost: 同じプログラムの旧暗黙moveと新操作を比較する。追加call/clone/allocation、generated Future frame、compile時間を必要な範囲で測る。文字列一致だけで意味同値を保証しない。

移行時はサンプルを分類してから書き換える。普通の関数引数まで機械的にmoveで包まない。入門はPythonとの比較、動く短い例、出力、move後再利用の誤りと直し方を日英で揃える。移行前の実行記録を残し、作業branchの例を現行公開版で動く例と説明しない。

## S1/S2: task結果の境界

[ADR 012](adr/012-task-result-handles.md)の初版意味論を設計採用した。全T正常出口await/discardとsticky faultは、今回の自律判断の委任による新しい詳細判断であり、旧承認から必然だったとは扱わない。現行spawn/Scope、compiler/runtimeとtest期待は変更しない。[設計](task-result-handle-design.md)のAPI名・構文・生成bridgeは未実装の接続候補である。

```text
async with scope:
    task = spawn fetch_user()
    received = await task
```

この構文例は使用可能なHighではない。Task[T]の受取はactual join後の外側Result[T, TaskFailure]とし、TがResult[U, E]なら二重Resultを保つ。普通の業務Errだけでは兄弟を止めない。

| 採用する境界 | 先行接続と必要な観測 |
|---|---|
| scope所属/一回受取 | handleは非Copy/nonClone/nonshared。awaitでconsumeしCopy結果も二度目を拒否。同scope localのmove aliasは責任を移し、scope外・関数・field/container/wrapper・他taskへのescapeを拒否 |
| 全T正常出口 | unit/Copy/Resultともawait/discardを求める。branch/loopとaliasの義務を追跡し、body Err/panic/親取消はcleanup。一般ownedや受取後Resultの未使用検出へ広げない |
| sticky故障 | panic/予期しない取消/legacy Err/protocol故障をprimaryに保持。兄弟abort→全actual drain後に外側Err、受取Err処理後も出口Err。bodyの元Eは後続faultで置換しない |
| 唯一join owner | ScopeがJoinSetを所有。ticketをentry key、native ID→ticketは未joinのみ。Ready→record間await無し、fake-ID再利用で未受取recordの混同拒否、receive/drain取消後の再開 |
| 受取/放棄 | sender Readyをactual joinとしない。receiptはScope強参照無し。未poll/Pending取消でhandleを復活させず、discard/T Dropは子停止・join/close完了を返さない |
| 親退出 | body Errはlocals退役→abort/drain→元Err伝播。親Future Drop/unwindはabort要求のみ。non-yielding・任意Drop panic・外部副作用の限界を残す |
| Supervisor | S1は旧statement spawnとterminal→HTTP取消を維持。S2で明示service fault昇格または親body Errへ接続。内側業務Resultへの機械置換はしない |

実装順はmove V1/V2→S1→S2→公開Pool/Tx。S1はtests-only→compile RED→小さい私有runtime bridgeから始め、結果通知だけのpublication前倒し版のruntime REDも保存する。Taskのscope依存awaitが現生成構造で表現できるかは未検証である。sealed Spawn/Await/Discard plan、scope label、元のview/cleanup anchor、Error変換を維持し、現bodyを全面async wrapperへ置き換えない。

typed結果channelと均一join recordを分ける接続を比較する。独自GC、async destructor、observer/追加JoinHandle owner、Any/downcast/unsafe、general region/effect checker、任意Future保存、新依存/default/timeoutは追加しない。現shutdownが後続Err/panicを返さないことから、新primary/related recordはabort_all＋ID付き手動drainで検証する。

正常/業務Ok/Err、fault、全T未受取、再await、escape、兄弟継続/取消、body Err/panic/Drop、故障後bodyをbarrierで観測する。sender通知、取消要求、Future Drop、actual join、結果受取を別eventにし、私有prototype成功を公開language完成と数えない。child側送信失敗Tとparent側buffer TのDrop、channel/entry allocation、長いbodyの完了未join/未受取保持、Future frame、retireは未検証の測定対象である。

scoped_tasksの一部runtime stubをTokioの終了実証としない。scope_runtime_contractは実runtimeを使う。旧service例の成功もSupervisor terminal fault→実HTTP停止の専用oracleではない。このbridgeとterminal連携を先行回帰で確認し、S2で明示移行を検証する。既存回帰と4 OS、[公開capacityの未解決ブロッカー](sqlite-capacity-decision.md)を別に記録し、公開Pool/Tx acceptanceやPhase 5を完了扱いしない。

## 移行前の監査と実装の進行条件

基点mainで関連する既存7 suite・36件を実行し、成功した。内訳はownership 14、shared_field_moves 4、copy_capabilities 2、result_discard 4、async_value_types 6、scoped_tasks 5、scope_runtime_contract 1。これは移行前の基点mainの契約の検査で、新しいmoveやTaskの実装成功ではない。

実行コマンドは`cargo test --locked -p nagic --test ownership --test shared_field_moves --test copy_capabilities --test result_discard --test async_value_types --test scoped_tasks --test scope_runtime_contract`。Linux、既存debug/test profile、offline依存cacheで実行した。source treeは`117533295a2bbd16d12d413cb00f72076d17133e`、raw log SHA-256は`e8bf0664eb1287ef05f38fb4896ce6bce6c416ca10124625d2b15ff74b4b500f`。

さらに[9例の移行前検証](value-task-audit-results.md)を実行した。High/手書きLow18入力と生成保存Low6入力のcheckは受理18・期待した拒否6。正常6例をHigh/保存Low/手書きLowでbuild/runし、18実行の出力が一致した。既存Copy class/enum・nullable・view、async関数別名、ユーザーのmove識別子、Result/sharedのnonCopy、借用中move、所有view containerの移動を確認した。新APIの実装・コスト検証には数えない。

V1/V2の意味論は確定済みで、既存設計に沿う具体APIの選択も今回の指示で認められた。同じ大枠を再質問せず、仕様→先行test→High/Low check→生成→Docs移行→全回帰→4 OS CIの順で進める。#85/#86はユーザーがmainへ反映済み。move実装の#87は最終head `5985e1b`から変更せず、ユーザーがmain `9ba4a10`へマージしたことを確認した。この設計・試作の別branchはmain向けdraftとし、こちらではマージしない。moveの完了後はS1、S2の明示サービス故障移行、公開SQLite Pool/Txの順に進める。S1の二つの確認待ちはADR 012の詳細採用で解除した。API接続・実装上の成立を先行テストで検証し、大きな新規意味論が必要なら理由と代替を記録する。merge/releaseはこの設計差分では実行しない。


## S1 Stage 1: 先行REDとprivate bridge（今回の一区切り）

[結果](task-bridge-stage1-results.md)にnative 16群、runner 3群、High/手書きLow56入力の照合を記録した。50の新Task入力はparse REDのまま。cfg(test) private bridgeのみを追加し、checker/codegen/旧Scope/SQLiteは変更していない。完了通知と実join、receiptとsticky fault、表示messageと元Error causeを分ける。コストは未測定。

この工程で止めて報告する。次は最新head・4 OS CI確認後、checkerのscope所属・一回消費・正常出口義務からsealed planへ進む。旧spawnの一括移行、公開Pool/Tx、merge/release/版更新は開始しない。
