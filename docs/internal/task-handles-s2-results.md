# Task S2: Supervisor結果の親への接続

2026-10-06。S1のPR [#88](https://github.com/disnana/Nagi/pull/88)は最終head `08e90c6984e689f7d026b3ff40277b9898ab4c68`で検証し、main `aee1987a7ede5eeebb5e253afd65cb5ede327dde`へmergeした。両treeは `bc62c787471d9e4481e36c0cca6e2ed294cebbc1`。[checks](https://github.com/disnana/Nagi/actions/runs/37540011861)・[website](https://github.com/disnana/Nagi/actions/runs/37540011529)は成功。4 OSのchecker110/110、native6、runtime17、public3、doc9、配布Task三構文/12拒否、Linux workspace934成功・failed0・費用用ignored1を観測した。未解決review threadは0。まだリリース済みではない。

S2は別branch `feat/task-service-result-propagation`で作業中。compiler/runtime、Cargo依存、公開API・意味論を変更せず、既存awaitと親のtryでservice故障を接続する。新しいfault昇格操作、業務Errの自動故障化、SQLite公開化は追加しない。以下はローカル検証の現在地。4 OS・最終回帰・指摘修正後の独立確認・mergeは追記するまで未完了。

## 移行と契約

[service High](../../test-nagi-code/library-examples/supervised-service/main.nagi)と独立[手書きLow](../../test-nagi-code/library-examples/supervised-service/main.low)はmonitorを`Task[Result[unit, Error]]`として保持する。awaitは`Result[Result[unit, Error], TaskFailure]`を返し、親がOk(inner)をtryする。Supervisor terminal Errは親body Errとなり、HTTP取消要求→直接の子の実join→元Error返却へ進む。HTTPは旧spawnのfail-on-Errを保つ。先に観測したHTTP faultは正当にscope primaryとなり、競合時に常にmonitor Errorが勝つとは保証しない。

正常なactor shutdownでは内側Okとなり、HTTPは503を返しつつ独立したHTTP終了まで動く。discardは受取放棄なので、内側terminal ErrだけではHTTPを止めない。これを正当なNagiの弱い移行反例として固定し、一般Result Errをfaultへ変換しない。

WebStateはActor/Controlのみを持ち、Supervisor contextをHTTP stateへ追加して解放と終了待ちを循環させない。外部のArc保持をTaskが回収する保証はない。同期親Dropはabort要求までで、直接の子・nested handlerの非同期close/実join完了を返せない。

## 先行oracleとnative

[専用oracle](../../compiler/tests/support/task_service_s2.rs)はHigh・保存Low（元High削除後）・独立手書きLowから生成Rustをbuildし、実actor、実serve_listener、socket、barrierで7ケースを実行する。手書きRustの観測adapterを使い、生成Rustは修正しない。

| ケース | 観測 |
|---|---|
| 正常shutdown | actor.shutdown Ok、monitor終了、HTTP503継続、独立停止後scope Ok |
| terminal Err | restart intensity元Error保持、親tryでErr、HTTP外部stop signal無しで停止 |
| discarded monitor | 元Err生成、HTTP503継続、有限の独立停止後scope Ok |
| HTTP outer Err | monitor待ち中に旧HTTP faultを受取、TaskFailure LegacyError、元HTTP Error保持。取消後のactorをControl.shutdownで別にdrain |
| monitor panic | actor cleanup後panic、TaskFailure Panickedを表示してもscope Err、HTTP取消 |
| 同期parent Drop | pending実handler確認、子HTTP Drop gate中でも親Dropは戻る。gate解放後handler/actor cleanupを別に観測 |
| 外部context保持 | worker STOPPED後もcleanup/monitor Pending、外部Arc解放後shutdown Ok。Taskは循環解決/GCをしない |

正常/Err終了では親が戻った直後にHTTP Drop済みを直接assertする。closed()で後から待つだけのoracleへしない。親Dropケースは別eventで観測する。non-yielding処理・任意Rust Drop/panic回復・外部副作用rollback・全nested handlerの非同期close保証には広げない。

初回native buildはadapterのErrorKind PartialEq仮定とEvent private field参照、修正後はunderscore lock lintで失敗した。Rust観測fixtureだけを修正し、Nagi checker受理後の生成不具合とは扱わない。手書きservice Lowの初回class表記もparse失敗し、正しいrecordへ修正した。失敗原ログを成功件数へ数えない。

## 公開例と現在の検証

[task-results](../../test-nagi-code/library-examples/task-results/README.md)はmove、一回await、内側業務Err、unit discard、scope後の成功表示を示す。[library verifier](../../scripts/verify_library_examples.py)は両sampleについて三構文のcheck/build/nativeを区別し、保存Lowは元Highを削除したコピーで実行する。成功exeはCLIのnative:から取得し、Cargo cacheの固定pathを実行しない。

ローカルtask-resultsは3構文成功。serviceも3構文で実HTTPの200/409/400、正常shutdown204、停止後503、Linux SIGINT正常終了を確認した。Windowsではprocess terminateによる停止検査で、graceful Ctrl+Cの実証とは数えない。4 OS CIへ両例の三構文を追加した。

Sol Highは自身のnative全3構文×7ケースと両sampleのchecked本文一致を確認した。HTTP Drop待ちが早いparent returnを隠す可能性と、spawn/await型のDocs表記を指摘し、直接assertと型の分離で修正した。修正後のSol High独立読戻しは完了、未解決指摘0。全workspaceは95 result block・935成功・failed0・費用用ignored1、fmt/clippy、checker110/110、全library15検証、website92頁が成功。4 OSはまだ未完了。原ログとsource/hashは[保存artifact](../../benchmarks/results/task-handles-s2-2026-10-06/README.md)へ集約した。S1最終CI原ログは現セッション`/tmp/nagi-s1-final-ci/`。PR headのCIは別に読む。

次はS2の全回帰・日英/リンク・独立確認・4 OSを完了してmainへ反映し、既存リリース手順で0.1.11を準備する。API差分/migration、全例、配布、版更新と最終独立reviewは別工程。公開Pool/Tx、条件付きshared actor message、一般Future保存、回復可能fault APIは今回のTask release blockerではなく未採用/別工程を維持する。
