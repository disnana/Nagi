# PR #87 独立実装レビュー

2026-10-06。対象は公開固定head `e4089735de35e5f8496a1191b870e7b5f7edffc9`。読取時のlocal `1727679c7f05410f38d4d6a9c287fa272fc6c529`と双方のtreeは `b27db5899e373fdc1c2d3418c4df004a3462bf77`で一致した。`origin/main`は `7d2d96a8bdba191e456f796bdf477b95bc34c57e`で、固定headのmerge-baseも同じ。AGENTSと契約・差分・生成harness・保存検証記録を読んだ。repo編集、Cargo、CLI/nativeの再実行、branch操作、追加agentは行っていない。GitHub全CI成功・reviewthreads 0はroot提供情報であり、今回独立取得した事実ではない。

## 実装の結論

読取範囲で未解決P0/P1、移行を止める導入TODOは見つからなかった。これは有限のコード・試験レビューで、全プログラムや全環境の証明ではない。

- `check.rs:2694`の新規拒否は通常AssignのRHSが解決済み `NameResolution::Local` のNameで非Copyの場合だけ。引数・return・field/index・matchのconsumeへ義務を拡張していない。括弧はparserで同じNameになる。BorrowedLocalは新診断の対象にせず、従来の借用拒否へ進む。Copy表は変更していない。
- canonical `std.ownership.move`はarity 1、明示型引数なし。`check.rs:1003`で期待型を入力へ渡し、emittable→consume→hold_valueを適用する。Futureを保存可能な値へ見せかけず、対応済みasync関数aliasのCopy/provenanceを保持する。利用者の同名関数やlocal shadowはclosed operationの意味にならない。
- `stdlib.rs:182`の私有WholeValue descriptorをview origin/content depth/temporary判定、async alias、typed constant評価が使う。ResultMapErrorのsuccess保持は別descriptorで残している。名前のwhitelistや全Callの無条件透過になっていない。
- `check/checked.rs:579`はcanonical DefId、引数個数・typearg、Passing::Move、入力/結果型一致、WholeValue descriptorを検証して私有planへ封印する。公開OperationInfoのfieldsとresource集合は維持する。
- `emit.rs:567`はそのplanから `::std::convert::identity(operand)` を一度生成する。Nagiの型・consume・origin判断をRust関数名に丸投げしていない。新Nagi runtime helper、unsafe、暗黙cloneを追加していない。Rust標準関数のby-value入力・結果がplaceを値temporaryへ変える。
- Copy aggregate比較でmoveの入力placeを再借用する旧14行の修正は除去済み。裸field/indexのPartialEq loanは既存のcomparison_originsで維持し、移動値中のview loanはhold_value/originで維持する。値のmaterializationと参照元情報を混同していない。
- `constant_eval.rs:229`はWholeValueのtyped Evaluationを返す。ゼロ除算、signed MIN/-1、ProfileDependentを一般CallのDynamicへ消してrustcへ押し出す変更になっていない。

Rust標準identityの根拠は[公式source](https://doc.rust-lang.org/stable/src/core/convert/mod.rs.html)の `#[inline(always)] pub const fn identity<T>(x: T) -> T { x }`。call命令ゼロや全profileでコスト不変という保証は導けない。

## 試験が観測しているもの

`compiler/tests/explicit_moves.rs`は9群。High→独立保存Lowと手書きLowのnativeは正常・lifecycle・Copy比較の3群×3入力で計9 build/run。Highを削除した後の保存Low再checkもある。生成Rustをpatchせず、実nagi-runtimeへCargoでリンクする。root Cargo.lockをseedし、更新後の依存(name, version, source)が元集合内であることを検証する。fixture自身の0.0.0 package以外の版更新を許さない。ビルド/実行のtimeoutは失敗としchildをkill/reapする。

| 要求 | 読取したoracle |
|---|---|
| owning nonCopy local通常代入拒否 | str/List/shared/owned/nullable/Result、括弧・型注釈・再代入・branch・loopの11入力をHigh/手Lowで元位置拒否 |
| 明示move・元再利用拒否 | canonical/alias/qualifiedとnested move正常、use-after/twice/partial/branch/loop異常。shared fieldとBorrowedLocalの旧拒否も維持 |
| Copy/freshが自然 | 18型＋期待型推論のcheck、Copy class/enum/nullable/view/functionとfresh constructorのnative |
| 関数への入力・返却 | explicit transferをユーザー関数引数へ渡す正常native、各関数のowning値裸returnを含む |
| borrow/shared/入れ子 | view中owner move拒否、shared field拒否、nested List/Option/Result view escape拒否と正常native |
| High/Low一致・Nagi段階拒否 | reject helperはsource::load＋Nagi checkでline/reasonをassert。nativeにはHigh・独立保存Low・独立手Lowを渡す |
| Clone/値 | String/Vec pointer維持、Arc ptr_eq・終了時strong_count、CloneMarkerのevent不在、実値一致 |
| Drop・失敗・取消 | replacement、RHS Err/panic、旧値Drop panicの厳密event列。未pollではbody無実行、Pending後の取消Drop順とcaller owner生存 |
| by-valueとtemporary | len(move(Vec))でstatement後かつ次capture前にDrop。単一/nested move(view(temporary String))が同式内で生存。Copy Option/unit/scalar field/index比較の移動値正常と裸place拒否 |
| constant | canonical/alias、8整数幅の `/` `%` とzero/compoundzero、signed4幅のMIN/-1をNagi診断。profile依存overflowはHigh/保存Low/手Lowのcheck受理 |

conformanceは42入力と独立harness。追加4入力はimportなしのimplicit assignment拒否とcanonical explicit move正常をHigh/手Lowで分ける。grammarは `index % 18` なので通常24件/CI256件に全18形が含まれる。新5形と既存view-container形はreinitialize、branch、loop、Result/Option payloadを含み、host側の有界算術oracleで値を比較する。これは任意のgrammarやcontrol flowの網羅ではない。

既存fixture差分は所有local転送をmoveへ変更し、import loader/canonical functionのlookupへ移行している。field転送や引数/returnへ一括move挿入していない。diagnosticの行移動はimport追加を理由に記録し、借用理由・use-after・flow facts・Drop列のassertは残っている。

## CI・fixtureと検査の限界

`.github/workflows/ci.yml:232`にexplicit_movesとconformanceを含むnative回帰が登録され、installationも別stepに含まれる。compiler-contracts workflowの有界生成256件とseed matrixも確認した。rootの固定head全CI成功情報と整合し、今回のsource上に新規skipや例外seedは見当たらない。

installationの変更はtest-only。exclusive create成功したdirectoryだけ所有し、copy済みimmutable seedをMutex<Weak<Seed>>からArc共有、個々のdistributionへ同temp filesystemのhardlinkを作る。実行中inodeを別fixtureがcopyのため書込みopenする経路を除く。各distributionのcurrent_exe検索・cwd・NAGI_ROOT診断と独立pathは維持し、最後の所有者Dropを新oracleが観測する。全test直列化や成功までのretryではない。独立probeのcopy 205/800、hardlink 0/800と実旧test失敗の記録は原因を支持するが、OS内部のFD相互作用をtraceで完全証明したとの記載にはしない。

保存ログのfull90 suite/899成功、clippy/fmt、10project19実行はroot実行の証拠を読んだ。最初に引用した93 suite/902は子process summaryの重複であり、以下の独立再集計に従って訂正した。今回自分が再実行したとは数えない。特定Markerでclone/Dropを観測した結果を、全payloadのallocation-freeや非同期frame不変に拡張しない。Arc終了時countだけで一時的なcloneを一般に検出できる訳ではないが、今回emitterのidentity経路そのものにcloneはない。ProfileDependent overflowのmove専用群はcheckのみで、各profileのnative挙動まで単独証明しない。

追加するなら既存NORMALの3-source native内に次の3点が小さい。rootへ既に推薦した。現headでの確認不足であり、確定不具合ではない。

1. `move(Copy scalar)`後の元parameter再利用を合計値で観測する。現check群は元値returnするが、native copy_shapesは別aliasをreturnする。
2. nonCopy値を裸のユーザー関数引数へ渡し、裸returnする正常native。現在のexplicit引数例に対し、義務を拡張しない保持oracleになる。
3. Resultの非Copy match payloadを裸returnする正常native。同じ狭い規則の保持を実行まで確かめる。

## 次の依存順とS1判断

`value-task-implementation-plan.md:148`の順はmove完了→S1→業務Err/故障分離とS2サービス移行→公開SQLite Pool/Tx。S1の新handle経路は初めからResultをTとして返し、業務Err分離なしの「handleだけ完成」は採らない。旧statement spawnとSupervisor terminal Errのfail-fastはS1で保持する。SQLiteのprivate adapter/多接続/budget試験成功はpublic registry・capture・APIの完成ではなく、任意に巨大なcapacityと公開Tx task transfer等の未解決を残す。Phase5/AuthScopeは今回へ巻き込まない。

全Tの正常退出でawait/discard必須と、受取故障をmatchしてもScope faultがstickyという二点は、ADR011/Q006から一意に導ける既決事項ではない。once-onlyはat-most-onceであって、通常のnonCopy localは自動Dropできる。現runtime Scopeにはsticky stateがなく、fault join後の再joinは空setならOkになる。従来Nagiにはその回復操作の公開入口がないが、これをsticky実装済みと説明してはいけない。

ただし今回rootから伝わった「安全・自然な具体判断を自律決定する」権限は、旧文書の未決記載だけを理由に再確認待ちへ戻す必要をなくす。次のA+Aを、新規採用判断として先に記録し先行REDへ進める推奨。既決の業務Err分離を再質問しない。

| 判断 | 推奨A | 最小の別案Bと影響 |
|---|---|---|
| 正常未受取 | 全T共通でawaitまたは明示discard。unit/Copyでもhandleはonce-only。異常退出はScope cleanup | handle暗黙Dropを許しScopeがjoinする。checker義務は小さいが未受取業務Resultを黙って捨てられ、型でmust-useを分けると型変更で検査が消える |
| 子fault | Scopeに最初の観測faultをsticky記録し兄弟abort/drain。受取Errの診断後も出口Err。業務Errは内側Tのまま | 対象の子faultを回復可能にし兄弟継続。未受取fault、別子fault、legacy Err、ack後の出口を別途定義する必要がある |

A+Aは結果責任とScope責任が一つの規則で説明でき、旧service faultの退出保証を消さず小さい初版にできる。回復可能なtask faultを将来足すなら明示policyと試験にする。全T必須は「業務Errを必ず処理した」保証ではない。awaitで得たResultをbindingするだけの現規則や明示discardを同時に全面変更しない。

## S1 runtimeで先に固定する条件

- Scopeが唯一JoinSet owner。handleはtyped receiverと小さいreceipt/ticketを持ち、Scopeへの強参照や第二のJoinHandle/observer taskを持たない。borrowed owner検査をRust最終traitだけへ送らない。
- Scope内独自ticketとnative IDを分ける。Tokio IDはjoin後に再利用可能。native ID→ticketは未joinの対応だけに使い、完了未受取recordの主キーにnative IDを使い続けない。
- join Readyでsetから除去した直後に、record publicationまでawaitを挟まない。受取Future取消で結果やfault記録を失わない。sender Ready/Dropはactual joinの証拠ではない。
- abortは要求、drainは終了確認。`shutdown()`は後続panicを捨てるので関連faultを保存する新経路はabort_all＋手動ID付きdrainを使う。非yield処理や親FutureのDropで同期join完了を約束しない。
- 受取Futureを生成した時点でhandleはconsume済み、未poll/Pending Dropでも復活させない。生存Scopeの終了責任は残す。body Future自体のDropでScopeも失われる場合とは試験を分ける。
- sender失敗時T、receiver中T、JoinError panic payloadのDropは任意Rustでpanicし得る。primaryを先に記録し、lock中・二重unwind中の万能回復を約束しない。discardはTのclose成功確認ではない。
- 完了未join・未受取出力はScope操作まで保持される。active task数だけでメモリ上限を主張しない。consumed/discardedかつjoin済みrecordを退役し、診断用にFuture/Tや全履歴を残さない。channel/entry/Future frameは実測対象。
- Supervisor terminal→HTTP取消の専用実socket回帰を先行する。既存業務409や正常shutdownだけで保証済みとは扱わない。新Task[Result]へmonitorを機械的に替えるとterminal Errが業務値になりHTTP停止を失うので、旧spawnをS1で維持しS2で明示昇格/親body Errへ移す。

一次資料はcached Tokio 1.53.1 `/workspace/toolchains/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tokio-1.53.1/src/runtime/task/id.rs:5`、`src/task/join_set.rs:300`（ID付きjoin/取消安全）、`:371`（shutdown）、`:459`（abort_all）。公式は[Id](https://docs.rs/tokio/1.53.1/tokio/task/struct.Id.html)、[join_next_with_id](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html#method.join_next_with_id)、[shutdown](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html#method.shutdown)、[abort_all](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html#method.abort_all)。依存source読取であり、依存自身の試験実行ではない。

## 後段44c38b7の追加レビュー

rootの依頼で `44c38b7687d2b95830f673f7d57c29745dc9e567` の追加差分も読んだ。固定公開headへの変更はexplicit_moves 31行と日英Docs4ファイルだけ。core5files、runtime、Cargo、workflowの差分は空であることを確認した。

上記3推薦は既存NORMAL_HIGH/手書きLOWに同じ3functionsとして追加済み。Copy inputはtransfer後に元parameterと移動結果を足して42を返す。plain argument/returnは裸 `text(value)` を渡す。ResultのOk payloadは裸returnし、Err枝はfresh文字列を返す。native assertionsは後者二つの実値とString pointer一致、Err枝も検査する。High/独立保存Low/手Lowの既存harnessをそのまま使い、元assertと9群/9native数は減らしていない。handLowのmatchは現行構文の `case Ok(payload) { return payload; }` に修正済みで、Rustの `=>` は残っていない。

この追加にP0/P1や過剰な義務拡張は見つからなかった。日英Docsは「move後にcopyを足す」のではなく「move代入をcopyへ置換」、元binding再初期化、Copy move後の再使用を説明し、狭い対象と未リリース状態を維持する。根拠を越えた性能保証や新APIは追加していない。

当該sourceのexplicit9群成功の後、rootの全workspace成功を原ログから読み戻した。正しい集計は90 suite/899であり、初報の902は下記の重複計上だった。これは固定e408のCI成功とは別sourceの結果であり、更新後の公開CIまで成功したとは数えない。3推薦の観測不足は44c38b7ではsource上解消済みである。

## 上位実行の再集計と仕上げ文書レビュー

rootがP2として見つけたsummaryの重複を独立確認した。`compiler/tests/graph_render.rs:227`はcurrent_exeを `--exact process_tests::image_child --nocapture --test-threads=1` で起動し、`.status()`で親stdoutへ子出力を継承する。正常・renderer失敗・renderer欠落の3testが同helperを呼ぶ。各子はfiltered 8の1testを走らせ、そのstatus成功と親側のfilesystem oracleを確認する。独立した上位Cargo suiteが3つ追加された訳ではない。

各原ログのRunning/Doc-testsでsectionを切り、そのsection最後のsummaryを一つずつ選択し、選択した全summaryのfiltered 0も確認した。結果は次の通り。

| 原ログ | 上位suite | passed/failed/ignored | SHA-256 |
|---|---:|---|---|
| `/tmp/nagi-explicit-move-proof/full-cargo-final-stable.log` | 90 | 889/0/0 | `e37139cbe01246db97f1f7b354a12c50c58bdc331a887b5fbad2a460ec04174c` |
| `/tmp/nagi-explicit-move-proof/full-cargo-main-integrated.log` | 90 | 899/0/0 | `be40eb73ff366b232706bd01e123f0723a91044a5190cd1a56e67152e4f66330` |
| `/tmp/nagi-explicit-move-proof/pr87-readiness-full.log` | 90 | 899/0/0 | `a753c6b8f063b49ad7f0c6064d0e3ffcd3e5ceba23ba0c4272c2cefbd3481348` |

全3rawのgraph_render sectionにはfiltered 8の子summaryが3行とfiltered 0の親9test summaryが1行ある。最新rawは子の一行が `test result: okok. 1 passed` とinterleaveする。これをnaive正規表現が見落として92/901と数えたことも整合する。旧二つのraw SHAは公開e408に保存された原ログblobと一致し、原ログの書換えやtest削除ではない。集計以外の成功・検査範囲は変わらない。他の過去phaseの件数は各rawを確認せず一律3を引く修正をしない。

未commitのexplicit-move-readiness.md、explicit-move-results.md、compiler-testing.md、progress.mdの変更を読んだ。上位90/889・90/899への訂正、旧93/892・93/902という初報を残した訂正理由、最新source44cと初回公開e408 CIの分離、release/mergeの権限、S1の新初版判断とpublic SQLiteの未完成範囲は適切。新P0/P1なし。compiler-testingの集計規則はこのcaseだけの恣意的除外でなく、上位binary/doctestの最終summaryを数える一般的な境界になっている。

nonblockingな文言提案として、readiness/progressの「compiler/runtime/依存/CIのbytes不変」はexplicit_movesの31行変更を含むcompiler directory全体とは違うので「compiler本体/runtime/依存/CIのbytes不変」とすると厳密。rootへ共有した。fmt/clippy/Docs7native/website90成功はrootの報告を文書で確認したが、ここで再実行したとは数えない。

## 問題一覧

- **P0/P1: 読取範囲で未発見。** moveの実装を止める再現、導入TODO、CI登録漏れはなし。
- **非blocking観測補強:** 固定e408で推薦した3点は44c38b7で追加済み。追加差分の読取に問題なし。後段sourceの全回帰は原ログから90/899成功を確認、更新後の公開CIとは分ける。
- **P2集計訂正:** 旧93/892・93/902はgraph_renderの子summary3件を重複計上していた。正しく90/889・90/899。raw保存hashと全成功は維持。TMP監査自身の初報引用も訂正済み。
- **S1新採用判断:** 全T受取/discardとsticky faultは旧契約の事実ではない。最新の自律選択権限の下でA+Aを採用記録→先行REDへ進める推薦。
- **次段階の未実証:** Scope-only join/ticket・取消publication・T破棄・保持メモリ・terminal→HTTP専用回帰、公開SQLite保証。PR #87の成功へ繰り入れない。
