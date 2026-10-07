# Phase 4: private SQLite sessionの検証結果

2026-10-08追記: 以下は当時のprivate試作の記録で、source linkは公開移行前baseへ固定する。現在の公開本体・adapter選択は[公開runtime判断](sqlite-public-runtime-decision.md)を参照。

2026-10-06。main `f10cb64`（#80成功headとtree一致）を基点に、Q002／[ADR 010](adr/010-sqlite-transaction-boundary.md)の一接続・一Tx試作を実装した。public Pool／Txは未実装で、Phase 4のacceptance完了ではない。先行基盤は4 OS CI成功後、ユーザーが#81をmain `ff6f7d4`へマージした。版更新・releaseは未実施。後続の[adapter比較結果](sqlite-adapter-results.md)は別記録とする。

## 変更と責任範囲

`runtime/src/lib.rs`からcfg(test)でだけ[prototype](https://github.com/disnana/Nagi/blob/676576724829e45b077b58628bfe2417e6cf3673/runtime/src/sqlite_prototype/session.rs)を読み込む。runtimeのrusqlite 0.40.2へ承認済みhooksを明示追加した。native SQL解析・bind・metadata・Transactionはrusqliteを使い、unsafe、自己参照、SQL parser、追加wrapper、pool algorithmを追加していない。

専用workerのlexical scopeにnative Transactionを置き、外側のnonClone handleはbounded senderを所有する。worker／ledgerはsession senderの強参照を保持しない。未pollの終端Future、送信前取消、begin reply喪失、最後のsender DropでEOF cleanupへ進む。受理済みcommandはcaller取消でも処理を続ける。常設Authorizerはprepare／step／reprepare／finalizeまで保持し、privateな管理区間でだけnative終端を許す。

native TransactionのDropは終了結果を捨てるため、DropBehavior::Ignoreを選び、明示終端のResultと消費後Connectionの状態を観測する。明示rollback失敗を隠れたDrop retryで成功にしない。COMMIT結果とcleanup causeは別に保持する。native closeとthread joinは別barrierで、通知だけをjoin完了としない。試験watchdogは失敗を早く検出するためで、公開timeout値や成功oracleではない。

このDriverは試験用である。sender cloneとcloseの全admission race、clone共通Pool、接続数、取得期限、Options／Parameters builderを実装していない。RustでTxをSend可能にしたことをNagiでtask転送可と扱わない。Nagi側のaffine／shared／capture制限は今後のchecker配線が必要である。

## 失敗から固定した契約

| 観測 | 共通原因と対応 | 分類 |
|---|---|---|
| tests-only `7accaec`で未定義Driver等、compile exit 101 | 実装前のRED。SQL契約の失敗ではない | 未実装段階 |
| COMMIT／cleanup両失敗、EOF rollback失敗でoutcomeがACTIVE | 終端失敗と継続可能なstatement Errを同じ状態にしていた。UNKNOWNとprimary／cleanupを別々に保持し、接続を退役 | private試作P1、修正済み |
| 自動rollback後にcommit成功を返す候補 | inactive native Txのfinishを一律成功にしていた。CommitはABORTED／ROLLED_BACK Err、Rollbackは終了済みcleanupとして成功。別negativeを追加 | private試作P1、修正済み |
| pragma／reprepare負例が単なるis_err | 別原因の失敗でも成功にできるoracle。policy外で同SQLの成功、Pragma deny action、管理flag、cleanupを対にした | 検証P2、修正済み |
| AFTER triggerのSQL Err後にINSERTがTxへ残る | native SQLiteの先行効果と、禁止PRAGMAの実行を混同した新test。承認していないstatement atomicity期待を訂正し、rollbackの観測を追加 | oracle／説明P2、公開契約変更なし |
| sender取得後にcloseする全race | private DriverはPool admission adapterではない。publicへ流用せず、starting worker／取得とclosingの共有ledgerで扱う | 後続adapterの必須検査 |

implementation commitは`3dff473`。最初の「Errなら変更0」失敗と同じbundled SQLiteの最小reproはローカル検証artifactへ保全した。BEFORE／AFTER triggerでPragmaはDeny、stepはSQLITE_AUTH、native Txはactive、finalizeは成功。先行INSERTはそれぞれ0／1行、明示rollback後は両方0行だった。SQLite sourceのAuthCheckとも一致する。独立Solは通常`INSERT OR FAIL`、RAISE(FAIL)にも同種の先行効果を確認した。

承認契約は普通のErr後もactive Txを継続できるもので、「全statement失敗で変更なし」ではない。暗黙savepoint、全Err自動abort、trigger／DDLの一律拒否は追加しなかった。新testの過剰期待を修正した理由と前後の観測を残し、禁止PRAGMA、先行効果、明示rollback、再利用を別oracleにした。既存の公開言語／CLI／High-Low／security・lifecycle期待、assert、seed、skip条件は変更していない。

## ローカルの実行結果

Linux x86_64、既存Rust toolchain／warm target、locked／offline Cargo。socketが必要な検査には実行環境のnetwork権限を使った。最初のsandbox内runtime baselineは既存HTTP bindがPermissionDeniedで失敗し、契約失敗と分けて記録した。修正後sourceを再実行して成功を確認した。

| 検査 | 結果・範囲 |
|---|---|
| [private native tests](https://github.com/disnana/Nagi/blob/676576724829e45b077b58628bfe2417e6cf3673/runtime/src/sqlite_prototype/tests.rs) | 22件成功。SQL制約、bind／shape／row decode、trigger／view／reprepare、自動rollback、EOF、満杯inbox、未poll／取消、COMMIT reply喪失、cleanup失敗／panic退役、native close失敗／join |
| runtime baseline | 151 unit＋5 doctest成功。上の22件を含む。旧Db／HTTP／Actorも実行 |
| [予定High／手書きLow入力](../../compiler/tests/sqlite_contract_inputs.rs) | 2 test成功。15組30sourceの構文と負例anchorを検査。semantic harnessは未配線、未知module拒否をTxのcompile-fail成功と数えない |
| workspace `cargo test --locked` | 92 suite・853成功、failed／ignored 0。上記runtime／parser検査を含み、件数を加算しない |
| fmt／workspace all-target clippy | 成功。clippyはlocked、`-D warnings` |
| fuzz smoke | 1000 mutation、95件check→Low／emit、16 bounded native、panic 0。既存pipelineのbaselineで、新SQLite APIを生成した検査ではない |
| corpus登録確認 | 38 corpus／16 linked harness。登録確認と実行を区別 |
| CI helper | 52件成功 |

検査ログは外部artifactに保存し、CIでは同じsourceの検査を再実行する。別snapshotの19件成功、途中のRED、最終22件成功を混ぜない。CI／siteの後続結果は下へ追記する。

## CIと未確認範囲

Linux checksの全Cargo suiteに新testが含まれる。4 OS package matrixへprivate SQLite sessionと予定High／Low入力の専用stepを追加した。Linux x64、Windows x64、macOS Intel／Apple Siliconの結果は、公開後のCI確認まで未確認。step登録だけで4 OS成功と報告しない。

新APIのHigh→保存Low→Rust build／run、Tx capture facts、native state収納拒否、dynamic SQL所有化、SQL opt-inのbind未検査表示、wrapper checkout／取得race、multi-connection、clone共通close、starting／idle／active／detached worker全joinは未完了。source mappingは既存の行単位で、新fixture columnを診断保証にしない。追加wrapperの[generic deadpool比較方針](sqlite-pool-adapter-decision.md)は、この一接続試作の後にQ004で承認された。adapterの実行成功はこの結果に含まれない。

性能改善は行っていない。prototypeはSQLを所有Stringへするため、そのcostを最終Static／Owned生成planの結果と混同しない。SQLite copy、owned reply／row allocation、Future frame、binary／compile timeは完成した縦切りで測る。既知のFuture +32 byteとCopy深さのP2も未解決である。

## 次の3項目

1. wrapper／未指定capabilityの判断後、一接続Manager adapterを同じcleanup／close oracleへ接続する。
2. public runtimeとcanonical registry、Tx実payload／Future捕捉、sealed SQL planを小さい縦切りで揃える。
3. 元位置付きnegative、High／保存Low／手書きLowの実Cargo、旧Db／4 OS、生成探索と性能測定でPhase 4 acceptanceを確認する。

## PR #81の4 OS CI

head `cfa65fa61de1f81d6acbebbfd4898541ba1ee5e1`の[checks run 37397252295](https://github.com/disnana/Nagi/actions/runs/37397252295)と[website run 37397251707](https://github.com/disnana/Nagi/actions/runs/37397251707)はattempt 1で成功した。Windows x64、Linux x64、macOS Intel、macOS Apple Siliconの各jobログで、private session 22件（失敗・ignoreなし）とparser 2件の実行を確認した。Linux全検査、VSIX、IntelliJ IDEA、PyCharm、merge gateも成功した。releaseはskipで、公開版への反映ではない。

#81をreview可能へ変更した。mainへのmerge・版更新・releaseは実行していない。このCIが確認したのは同headの一接続coreと30の構文入力であり、後続deadpool adapter・公開Pool／Tx・Future捕捉検査の成功とは区別する。Q004承認後のManager比較は別branch `feat/sqlite-pool-adapter`で進める。
