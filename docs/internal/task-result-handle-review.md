# S1 Task結果handleの独立レビュー

2026-10-06。読取基点は#87 head `e4089735de35e5f8496a1191b870e7b5f7edffc9`。独立Solレビューを、リポジトリ内の根拠と依存の公式資料へ整理した。対象は[設計候補](task-result-handle-design.md)、[Q-006](open-questions.md#q-006-spawn結果handleと業務errtask故障)、[ADR 011](adr/011-language-behavior-and-docs.md#async-0304-結果handleと失敗の分類)。コード・test期待・現行の保証範囲を変更せず、Cargoや新prototypeを実行した記録でもない。後述の二判断は今回の自律判断の委任に基づいて[ADR 012](adr/012-task-result-handles.md)で設計採用した。

## 採用方向と詳細判断

scopeが寿命と実joinを所有し、結果を一度受け取り、普通の子Resultの業務Errで兄弟を止めない方向は採用済み。旧statement spawnのfail-on-ErrとSupervisor terminal→HTTP取消は明示移行まで保つ。これは[DESIGN](../../DESIGN.md#失敗と並行処理の境界)と[英語版](../../DESIGN.en.md#failures-and-concurrency-boundaries)の方向と一致する。

初回レビュー時点では次の二判断を確認候補とした（設計候補`9ea6995`）。既承認の方向から一意に決まらないという評価は維持する。その後の「既存設計から安全に判断できるものは理由を示して自律確定」という委任に基づき、初版はA+Aを選んだ。新Taskの結果意図を全Tで揃え、receiveが観測した故障を出口で消さないための新しい詳細判断であり、過去の承認に必然だったとは説明しない。

| 判断 | A: 今回採用 | B: 初版で不採用 |
|---|---|---|
| 未受取 | unit/Copy/Resultを含む全Tで正常出口await/discard。異常退出はScope cleanup | handleの暗黙Dropを許し、Scopeはjoinを続ける。checker追加は小さいが未受取Resultも黙って放棄できる |
| fault回復 | fault観測→兄弟abort→全実join。primaryをstickyに残し、受取Errを処理してもscope出口Err | 明示受取した単一子faultを回復可能とする。兄弟継続・ack・未観測fault・legacy Errの扱いを別に定める必要がある |

at-most-onceはmust-consumeと別で、現行所有localは暗黙Dropできる。await後の内側Resultをbindingへ入れても業務Errを処理した保証にはならず、全面Result must-useは追加しない。[現Scope](../../runtime/src/concurrent.rs)にはsticky fieldがなく、故障を返した後の再joinは空setでOkになり得る。現Nagi生成がこの回復経路を公開していないことも、新awaitでの回復禁止が既承認である根拠にはならない。

## runtime設計に反映する条件

- ScopeをJoinSetの唯一ownerとし、Taskはtyped receiver・scope identity・ticket・小さいreceipt stateまでとする。receiptにScope/JoinSetへの強参照やowner callbackを持たせない。追加JoinHandle owner・observer task・Any/downcast・unsafeを必要としない案を比較する。
- `entry[scope_ticket]`はscope生存中に再利用しないticketで識別する。Tokio native ID→ticket対応は未join taskだけに持つ。[Tokio Id](https://docs.rs/tokio/1.53.1/tokio/task/struct.Id.html)は終了かつactive handle/set所有終了後に再利用できるため、join後の未受取recordをnative IDで識別し続けない。
- [join_next_with_id](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html#method.join_next_with_id)の取消安全性はPending中の除去を防ぐ。Ready後の除去→ticket解決→実join記録→未join対応除去→cause公開にはawait・任意TのDrop・ユーザーcallbackを挟まない。生存Scopeでdrain Futureを破棄してもowner/recordを残し、次操作で再開する。
- receiver Ready/closed、送信完了、abort要求、discardを実joinと同一視しない。[abort_all](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html#method.abort_all)後もdrainが必要。[shutdown](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html#method.shutdown)は後続panicを無視するため、故障記録を残す候補ではID付き手動drainを比較する。
- 送信失敗TのDropはchild側、送信済みbuffer TのDropはparent側で起き得る。成功受取後もTの終了責任は残る。discardや実joinを任意資源のclose完了とは説明せず、[Drop](https://doc.rust-lang.org/std/ops/trait.Drop.html)のpanic・二重unwind・abortからの普遍回復も約束しない。
- receive Futureの未poll/Pending Dropでhandleを復活させない。生存Scopeでの受取取消と、親Future DropでScope自身が失われる場合を分ける。同期Scope Dropはabort要求までで、actual join完了を返せない。
- background joinなしでは長いbody中に完了未join taskが残り、join後も未受取Tを保持し得る。メモリを実行中task数だけに限定しない。実joinかつ受取/放棄後にentryを退役し、channel/entry・未受取T・Future frameを測る。新capacityやtimeoutは追加しない。

## 小さい先行oracleと移行

private fake-ID seamでAのjoin後も未受取結果を保持し、新ticket Bに同じnative IDを割り当てる。A/Bのrecord・receiverを混ぜず各値を一度受け取ることを検査する。実Tokio IDの再利用を待つ試験にはしない。

業務Err後の健康兄弟barrier、sender Readyでもcleanup gate中はreceive Pending、target以外の先行join、receive/drain取消後の再開、Drop場所・回数を独立に観測する。全T義務とsticky出口は採用仕様の先行oracleとして固定する。通常ownedや受取後の内側Result全体にmust-useを広げない。watchdogはhang検出用で、gate解放・取消・可能なdrain・fixture cleanup後にassertする。

High/保存Low/手書きLowは[AST](../../compiler/src/ast.rs)、[parser](../../compiler/src/parser.rs)、[checker](../../compiler/src/check.rs)、[emitter](../../compiler/src/emit.rs)の同じscope/consume/cleanup factsへ接続する候補を検査する。現行の[実Scope回帰](../../compiler/tests/scope_runtime_contract.rs)と、[一部stubのscoped_tasks](../../compiler/tests/scoped_tasks.rs)を混同しない。

[service例](../../test-nagi-code/library-examples/supervised-service/main.nagi)の旧monitor/HTTP spawnは維持する。terminal Errを新Taskの内側業務Resultへ機械置換するとHTTP停止を失うため、S2の明示故障昇格または親body Errへの移行で扱う。[actor run](../../runtime/src/actor.rs)・[lifecycle](../../runtime/src/actor/lifecycle.rs)を根拠とし、terminal故障による実socket停止、reply業務ErrでのHTTP継続、正常shutdownを別caseにする。既存sampleの成功だけでこのterminal故障oracleを実証済みとはしない。

この改訂はS1詳細意味論の設計採用であり、実装・CI成功・main反映・releaseではない。#87公開headは変更せず、別工程の実装前レビュー用に保持する。bridge、join record、allocation/保持量と4 OSは未検証。[採用理由・接続条件](adr/012-task-result-handles.md)と[必要な先行validation](task-result-handle-design.md#先行validationと接続順)を正本とする。


## Stage 1後の独立確認

[Stage 1結果](task-bridge-stage1-results.md)のprivate sourceとrunnerを別途読取レビューした。元Errorを表示messageだけに置換する問題と、runnerのcanonical path不一致を指摘し、修正後を再確認した。legacy causeは元ErrorをArc内に保持し、Ready→record→cause公開後にnative JoinError payloadを破棄する。既存checkerの元行負例をHigh/Lowで直接確認するrunnerを残した。新ブロッカーなし。ただしレビュー担当はCargoを実行しておらず、Nagi Taskの全T義務/escape/codegenを完成とは評価していない。速度・allocation・Future sizeは後続の測定対象。
