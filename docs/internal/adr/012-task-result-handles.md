# ADR 012: scope所属の結果handleと故障の保持

状態: **設計採用・S1作業branchに接続済み・未リリース**。private runtime bridgeの先行検証は[Stage 1結果](../task-bridge-stage1-results.md)に記録する。2026-10-06の「既存設計から安全に判断できるものは理由を示して自律確定」という追加指示に基づく詳細判断。旧Scope、spawn、test期待、公開版を変更した記録ではない。API名・構文とruntime接続は[設計案](../task-result-handle-design.md)、検証の順序は[実装計画](../value-task-implementation-plan.md#s1s2-task結果の境界)に残す。

## 根拠と今回の判断

[ADR 011](011-language-behavior-and-docs.md#async-0304-結果handleと失敗の分類)は、scopeによる寿命・故障管理、一度限りの結果受取、普通の子Resultの業務Errを故障から分ける方向を採用した。全Tの受取義務とfault処理後の回復性までは一意に決まらなかった。[独立レビュー](../task-result-handle-review.md)でも二判断を候補として比較した。

今回の委任に基づき、初版は**全Tの正常出口await/discard**と**scope故障のsticky保持**を選ぶ。過去の承認に必然的に含まれていたとは説明しない。設計候補commit `9ea6995`での確認待ちはこの新判断で解除する。実装、実証、main反映、releaseは別に記録する。

| 選択 | 理由 | 今回採らない案 |
|---|---|---|
| 全Tの正常出口await/discard | 新Taskの結果bindingに受取/放棄の意図を求める。unit/Copy/Resultで規則を分けず、返却型変更で未受取の検査が消えない。結果不要の旧statement spawnは残す | 暗黙DropもScopeがjoinすれば寿命上は成立するが、未受取Resultを黙って捨てられる。Resultだけ義務化は型依存の例外を増やす |
| 故障はsticky | receiveで早く取り出した故障が正常出口で消えることを防ぐ。既に兄弟をabortしたscopeを、受取Errの表示だけで健康に戻さない | 回復可能な単一子faultは、兄弟継続/abort後のack/未観測fault/legacy Errを追加設計する必要がある。初版に回復APIを増やさない |

at-most-onceとmust-consumeは別である。全T義務は新Taskだけに適用し、通常owned localやawait後の内側Resultの未使用検出を広げない。現[Scope](../../../runtime/src/concurrent.rs)にsticky fieldが無いこと、故障join後の再joinが空setでOkになり得ることも保存する。現Nagi生成がこの回復を公開しないことを、旧契約にstickyが実装済みという根拠にしない。

## 初版の採用契約

ここでのTask、TaskFailure、discardは採用契約の呼称である。2026-10-06のS1再開では[接続判断](../task-handle-implementation.md)の `task = spawn work()`、canonical `std.task.Task[T]`、`discard` (unit)、`kind`、`message` (failure-origin view) を実装方針として確定し、先行負例から接続する。KindはPanicked/Cancelled/LegacyError/Internalの4値。scope選択は最寄りscope単位のsealed planに保持する。まだ実装完成・公開版での利用を示さない。

- Task[T]はscope所属、非Copy・非Clone・非shared。作成時と同じscopeのlocalでのみ保持・移動し、scope外、関数引数/return、field/container/wrapper、他taskへ逃がさない。一般region/effect checkerやFuture保存は追加しない。
- awaitはhandleをconsumeし、TがCopyでも一回だけ受け取る。未poll/Pendingの受取Future Dropでhandleを復活させない。正常binding/scope出口とloop継続では全Tでawaitまたは明示discardを必要とし、move aliasへ義務を移す。body Err/panic/親取消はcleanup経路で扱う。
- 受取はScopeのactual join後に外側Result[T, TaskFailure]。TがResult[U, E]ならResult[Result[U, E], TaskFailure]を保ち、flattenしない。業務Errだけでは兄弟を止めない。受取後のTは通常の所有権/Result規則に従う。
- panic、要求していない取消、旧statement childのErr、内部protocol故障はscope fault。最初に観測したfaultをprimaryとして保持し、兄弟abort要求→残るactual joinのdrain後に外側Errを返す。受取Errをmatchしてもscope出口は失敗し、関連faultはprimaryを置換しない。要求済みCancelledはcleanupと分け、実際に返ったpanic/legacy Errは故障として残す。
- body Errは元Eをprimaryとして返し、body localsの退役後に子abort/drainを待つ。後続child faultで元Eを置換しない。parent Future Drop/unwindの同期Dropはabort要求までで、join完了は返せない。non-yielding処理の強制停止、副作用rollback、任意Drop panicからの普遍回復は保証しない。
- discardは受取を明示放棄するが、子をdetach/停止せずScopeのjoin責任を残す。discard、sender Ready/closed、任意TのDropをactual joinや資源close成功と同一視しない。
- 旧statement spawnのunit/Result[unit, Error]とfail-on-Err、Supervisor terminal→HTTP取消を維持する。新Taskの内側Resultへ機械置換して停止連携を失わない。S2で明示service fault昇格または親body Errへの接続を扱う。

## ownerと記録の接続条件

ScopeがJoinSetの唯一ownerとなる。Taskはtyped receiver・scope identity・ticket・小さいreceipt stateまでで、Scope/JoinSetへの強参照や追加join ownerを持たない。scope内ticketをentry keyとし、native ID→ticketは未join taskだけに使う。[Tokio Id](https://docs.rs/tokio/1.53.1/tokio/task/struct.Id.html)はjoin後に再利用され得るため、未受取recordのkeyへ使わない。

join Readyによる除去→ticket解決→actual join記録→未join対応除去→cause更新にはawait・任意TのDrop・user callbackを挟まない。[join_next_with_id](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinSet.html#method.join_next_with_id)の取消安全性と、Ready後のrecord保持は別に検査する。生存scopeのreceive/drain取消後も再開可能とする。故障診断のために終了Future/Tを延命しない。

background joinなしでは、長いbody中の完了未join task、join済み未受取Tが保持され得る。実joinかつ受取/放棄後にentryを退役し、実行中task数だけでメモリ量を説明しない。送信失敗Tのchild側Dropと送信済みbufferのparent側Dropを区別する。新capacity、timeout、依存、default、独自GC、async destructorは追加しない。

## 未検証のbridgeとacceptance

新Taskのparse/check/保存Low/生成Rust/公開runtimeを接続した。private bridgeの歴史的検査は[Stage 1結果](../task-bridge-stage1-results.md)、公開言語の検証と未確認範囲は[接続結果](../task-handles-s1-results.md)に分ける。次の表はS1全体のacceptanceであり、接続開始だけを全項目成功としない。

| 対象 | 先行oracleと必要な確認 |
|---|---|
| Scope/Task bridge | 私有typed receiver＋唯一join ownerを先に試す。receiver Readyでもchild cleanup gate中はreceive Pending、Future Drop→actual join→受取を別eventで観測 |
| join record | target以外の先行join、receive/drain未poll/Pending Drop後の再開、故障primary保持。fake native IDをA join後の新ticket Bへ再利用し、A/B未受取recordを混ぜない |
| checker/生成 | 全T未受取正常出口拒否、await/discard受理、alias/branch/loop、Copyも再await拒否、escape拒否、元位置。High・保存Low・手書きLowからsealed planを経てRust build/run。既存scope label・Error変換・cleanup anchorを維持 |
| 業務値/fault | Result Ok/Errと健康兄弟barrier、panic/取消/legacy Err/protocol故障、match後も出口Err、要求取消と実fault競合、body Err優先。parent Dropはabort要求と別の終了barrierで検査 |
| Drop/コスト | child側送信失敗T・parent側buffer T・受取後TのDrop、channel/entry allocation、長いbodyの完了未join/未受取保持、Future frame、retire。alloc-freeやサイズ不変を前提にしない |
| terminal連携 | 旧経路のSupervisor terminal故障→実HTTP listener停止/handler cleanup、reply業務ErrでHTTP継続、正常shutdownを分ける。既存sample成功だけでは新bridgeを実証しない |

順序はbarrier、oneshot、Notify、独立eventで作り、sleepをoracleにしない。watchdog失敗でもgate解放・取消・可能なdrain・fixture cleanup後にassertする。私有prototype成功と公開言語完成を分け、全回帰/4 OSまで範囲を記録する。

依存順はmove V1/V2→S1→S2→公開Pool/Tx。S1は旧spawnを残して新経路を追加し、S2でサービス故障の明示接続を揃える。公開Pool/Txのacceptanceや[巨大capacityブロッカー](../sqlite-capacity-decision.md)は未解決のままで、このADRはPhase 5、公開DB保証、merge、版更新、releaseの実施記録ではない。
