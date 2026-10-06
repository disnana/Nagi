# コンパイラ・Rust境界の未決事項

## Q-001: 同じアプリの識別と実行ファイルの世代を分ける

状態: 2026-10-05にAを承認済み。PR0で発見。Phase 2の開発差分で、承認済みの内部path期待を更新した。検証とmainへの反映状況は[進捗](progress.md)を参照。

### 問題

新依頼のPhase 2は、成功generationをside-by-sideで保持し、実行中の旧binaryを上書きしないことを要求する。一方、既存テストは同じcanonical source/outを再buildした際、実行ファイルのcanonical pathが前回と同じであることを要求する。

当初のStop条件「既存テストの意味論上の期待値を変更しないと通らない」に従って確認した。回答では、同一binary pathは内部実装上の契約であり、承認済みgeneration設計に合わせて変更可能と明示された。旧assertを削ったり、generationを単なるsymlinkで同じ実体へ向けて隔離したことにしたりしない。

### 確認時の根拠

- [shared_target.rs](../../compiler/tests/shared_target.rs)の212行: 等価pathで再buildした成功binaryのcanonical pathを前回と比較する。230行: 既定/明示で同じcacheを選ぶ別buildにも同一pathを要求する。
- 同テストの195〜199行: stemにapp identityの16桁だけを要求する。世代名を追加すればこの期待も調整が必要。
- [project.rs](../../compiler/tests/project.rs)の351〜359行: `build/native-target/release`直下と現在のname形を検査する。
- [ADR 005](adr/005-native-artifact-identity.md)の「契約と採用案」: source/outの識別と生成位置維持を定める。ただし「同じ生成先への同時compile未対応」は#76の範囲・現在の制約であり、単独で将来機能を禁止する規範とは扱わない。
- [projects](../projects.md)は既存生成Cargo.lockの保持、`--out`と`build/<入口>/`の生成場所を説明する。generation移行でこれを捨ててはいけない。

### 選択肢

| 案 | 挙動・影響 |
|---|---|
| A. app identityを維持し、実行artifactだけ世代固有にする | `native:`は毎回成功generationの実path。等価source/out・既定/明示cacheは同じapp ID、別generationのpathは異なる。旧binaryは維持。依存cache・project cwd・既存out/lockは維持する設計を検証する |
| B. 同一canonical binary pathを維持する | 旧exeをそのpathで実行している場合、新exeの置換とWindows互換性に衝突する。run終了までbuildを止める等が必要となり、今回のgeneration条件を満たせない。Phase 2の条件の再検討が必要 |

### 互換性と必要なテスト変更

Aでは実行ファイルpath/nameの既存期待が変わる。app identityの決定性は保ち、generation identityとは別に検査する。`native:`を読むscripts/editor/distributionを確認し、固定nameを前提にする箇所を移行する。生成Low・Rust・Cargo.tomlの既存参照先と既存Cargo.lockの継承は別の契約として保持する。

現在のstdout・異なるappの分離・同一dependency cache・project cwdのassertは残す。追加するのは同一appの旧exe継続、別generation、並行build、failed buildでlatest不変、Windowsでの実行中exeと新buildである。

同一pathのassertをapp ID同一＋generation path相違へ変えるのは、実装ミスを隠すtest weakeningではなく、承認済みgeneration契約への移行である。公開意味論・CLI/API利用者契約・High/Low互換性・Guarantee Register・security/lifecycleの期待変更は引き続きStop。内部生成先・file名・pathの期待は、理由を記録して更新できる。

### 推奨案

Aを推奨する。旧app identityとcache共有を保ち、実際にrunするimmutable generationを明示できる。成功metadataはatomic更新し、run中の旧generationを上書き・削除・killしない。

[計画書](compiler-rust-boundary-plan.md)へ採用判断とworking rulesを反映した。app identity維持→世代別生成→build成功後のatomic latest更新を採用する。PR0更新CI成功後にPhase 1から順に再開し、Phase 2を先に実装しない。

## 後続Phaseで具体化する項目

### Q-003: AxumサンプルのContent-Type欠落時の受信policy

状態: 2026-10-06に案Aを承認済み。[ADR 009](adr/009-axum-rejected-body.md)に固定した。欠落時だけ4096 data bytes・読取開始から1秒のcooperative期限、415優先、未完ならcloseとする。正常Json処理と元client testは維持する。新policy値のStop条件に従って確認したもので、本文待機を標準HTTP全体へ広げる承認ではない。修正後CIの完了は別に確認する。

### Q-002: SQLite Pool／Txの初版APIと終了policy

状態: 2026-10-06にユーザーが選択1を承認。公開API・SQL制限・終了policyとruntime rusqlite hooksを採用した。[ADR 010](adr/010-sqlite-transaction-boundary.md)へ固定する。Phase 3の#80はCI成功後にmainへ反映済み。実装・検証の完了とは区別する。具体的な署名・所有契約・値・失敗policyは[API契約](sqlite-pool-proposal.md)、採用候補と不採用案・native APIの根拠は[調査](sqlite-pool-research.md)にある。

推奨候補は`std.db.sqlite`、owned Parametersの型別builder、affine Tx、worker-localのsafe rusqlite Transaction。旧Dbは維持する。必要な容量・timeout・begin modeは明示指定し、数値defaultを追加しない。SQLを自作解析せず、SQLite prepare・Authorizer・結果metadataを使う。

判断は3つに分ける。

1. module/resource/API・Parameters・行型・NULL/placeholder・required optionsの範囲。
2. runtimeのrusqlite `hooks`有効化と、新Txだけに適用する一文・transaction-control/PRAGMA等のSQL制約。[既存Rust wrapper](sqlite-pool-rust-reuse.md)の比較結果を反映し、追加crate/feature/版が必要なら別に明示する。hooks承認をwrapper依存承認と兼ねない。
3. cleanup確認前の再利用禁止、退役時の新取得停止、commit outcomeとcleanup failureの分離、close後の取消/timeoutの扱い。

上の3判断とruntime hooksは承認済み。同じ承認を再要求せず、ADRとfailing testsから進める。wrapperは別のQ004で承認した。safe一接続prototypeは#81でmainに入り、その範囲のcleanupを検証した。公開Pool/Tx、Txのcapture追跡、多接続は未完了。safe APIで成立しない場合は保証を下げず反例と代替案を示す。

### Q-004: SQLite Poolのwrapper依存と未指定capability

状態: 2026-10-06に依存とcapability表を承認済み。generic deadpool 0.13.1（managed／rt_tokio_1、default featuresなし）とdeadpool-runtime 0.3.1を比較試作へ追加し、既存Tokio／rusqliteの解決版を維持する。承認した表の値を、Q002の終了・転送契約とともに扱う。

[PR #82](https://github.com/disnana/Nagi/pull/82)のprivate一接続adapter比較は4 OS CIまで成功した。main `7999bab`へ反映済みだが、公開registry、NagiのTx捕捉検査、多接続、取得期限まで完成したとは扱わない。[main側の判断資料](sqlite-pool-adapter-decision.md)は依存選択の根拠として残し、承認と実装状況はこの記録を参照する。Q002/Q004を再び未承認へ戻さない。Tx／ParametersのDebug不可、Pool／Failureの状態だけのDebug、Failureと小さいenumのshared可は初版表の採用値で、公開checker配線は未完了。予想外の依存追加・版更新が必要なら差分を示して判断へ戻す。

### Q-005: 既存所有値の代入を明示する範囲

状態: 明示moveのAPI・Copy据置・狭い通常代入移行は追加のユーザー指示で確定。別branchで実装・検証中であり、公開mainには未実装。[ADR 011のOWN-04](adr/011-language-behavior-and-docs.md#own-04-既存所有値の代入)にbefore/after、理由、移行・検査を記録した。

canonical `std.ownership.move`（一引数、型推論、明示型引数なし）、現行Copy表の据置、右辺が所有するnonCopyローカルそのものの通常代入移行を採用した。[V1/V2の採用仕様](value-task-implementation-plan.md#v1v2の採用仕様)へ統合し、操作追加と狭い拒否を一つの別実装PRへ段階的に積む。新値生成、引数・return・field/index・try・matchの既存consume規則は維持する。Futureや入れ子のFutureはmove入力へ解禁せず、対応済みasync関数別名のCopyも維持する。

この文書PRと現在のmainでは非Copyの`a = b`を暗黙moveとして受理する。移行前の36件と正常18実行は監査証跡として保持し、別branchの実装成功やmain公開機能に読み替えない。merge・版更新・release、task結果handleの詳細は別工程とする。

### Q-006: spawn結果handleと業務Err・task故障

状態: 方向は採用、現行spawnとScopeは変更しない。[ADR 011](adr/011-language-behavior-and-docs.md#async-0304-結果handleと失敗の分類)に移行を分離した。

段階実装の対象になったが、[S1/S2の具体案](value-task-implementation-plan.md#s1s2-task結果の境界)は未決を残す。scope-owned join、once-only結果、業務Resultと外側の故障、Supervisorからの明示的な故障伝播を検討する。V1/V2の確認時にこれら全てを再質問せず、task実装へ進む段階で必要な意味論をまとめて決める。

結果を一度受け取るhandle、scopeによる寿命・故障の管理、業務Errだけでは兄弟を止めない方向を採る。型名・構文、消費規則、故障/取消型、複数故障、未受取Result、検出時点、scope外への持出しは未決。現行の子Errによる兄弟取消、Supervisor terminal ErrとHTTP終了の連携は、明示的な移行なしに消さない。親本体がtry等で退出する場合と、子が業務Resultを返す場合を分ける。

### Q-007: actorの条件付きshared message

状態: 方向は採用、型と容量・寿命の条件は未決。現行message/replyのshared拒否を維持する。[ADR 011](adr/011-language-behavior-and-docs.md#actor-01-条件付きshared-message)に移行・検査を記録した。

Send/Sync、内部可変性、容量課金、資源の保持、replyへの流出を決める。sharedという型名だけで許可しない。既存owned message、業務reply Errとworker故障の区別、取消/timeout後に受理済み仕事が実行され得る契約を保つ。

これらの未決項目で文書作業全体を止めない。構文や公開保証を実装する段階では、具体的な移行と失敗テストを用意してから判断する。周辺のoverflow、文字列index、class比較、Map key、mutable globalを、この判断の一部として追加決定しない。

### その他の項目

承認済みの契約と、残る設計・実装を分ける。未実装を未承認へ戻さず、承認を実行保証にも読み替えない。

| 項目 | 判断する時点 | 条件 |
|---|---|---|
| Pool容量・acquire/busy timeout・transaction開始mode | Q002契約は採用、公開実装は後続 | required Optionsとmode、既存Dbを変更しない方針は承認済み。新defaultや取得期限budget・巨大容量の検証に追加判断が必要なら根拠を示す |
| Pool/Tx module・API、worker session/lease | Q002/Q004採用、Phase 4の公開配線は未完了 | safe一接続試作を維持し、公開registry・capture・SQL・多接続を検証。unsafe/新driver/追加依存が必要ならStop |
| 旧Grant[P]とGrant[P,Scope]の互換性 | Phase 5以降 | Phase 4完了前に実装しない。arity変更・既存API削除は別途判断 |
| 任意opaque Rust resource・async callback | 計画外 | Rust API自動importや自己申告Contractを追加しない。必要なら別設計 |
| Txを捕捉したFutureのtask transfer | Phase 4設計・negative tests | Futureの戻り値型だけで判定しない。alias/return/Option/標準task起動を含むprivate capture factsを検証。一般effect/regionが必要ならStop |
| 世代snapshotと互換出力 | Phase 2実装・main反映済み | canonical outのwrite lock、app別metadata、check/lower並行とprojection途中失敗を検証した。外部workspace全体のatomic snapshotは対象外。公開版への反映は別に確認 |
