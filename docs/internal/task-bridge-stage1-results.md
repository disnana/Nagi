# S1先行REDとprivate Task bridgeの検証

2026-10-06。基点はPR #87最終head `5985e1b0c9f063f0a7153b3a79e8847f53765035`。ユーザーが#87をマージし、main `9ba4a104be6f65dba61eda0c7b1ef0f98c892cf7`のtree `6986c81b76649b9ab8f6bf4867270fea51ff35cd`が最終headと一致することを確認した。#87へ追加変更を積んでいない。今回の別PRはmain向けdraftとし、merge、版更新、releaseは実行しない。

## 今回実装した範囲

[ADR 012](adr/012-task-result-handles.md)のruntime接続を、[cfg(test)のprivate module](../../runtime/src/task_bridge_prototype.rs)で検証した。公開Scope、compiler本体、旧statement spawn、Supervisor/HTTPサンプル、SQLite、依存は変更していない。NagiのTask構文、型、must-consume、escape検査、Low/Rust生成への接続は**未実装**。

Scopeが唯一のJoinSet ownerとなり、Taskは型付きoneshot receiver、scope identity token、再利用しないticket、小さいreceipt stateを持つ。結果の通知とactual join、結果の受取状態とscopeのsticky failureを分けた。旧spawnのErrは構造化した元Errorをcauseへ保持する。公開TaskFailure・Error変換は後続の実装対象である。

[High/手書きLowの先行入力](../../tests/task-handles/README.md)は28組・56入力。未受取unit/Copy/非Copy/Result、二重await/discard、move alias、再代入、shadow、分岐、loop、scope escape、旧spawnとユーザーの同名関数を登録した。[runner](../../compiler/examples/task-contract-red.rs)はparse→name resolution→checkまでを照合する。negativeはchecker段階・診断fragment・元のprimary行を必要とし、parse拒否、import拒否、ICE、異なる行を成功扱いしない。保存Lowの生成・Rust build/runの新Task conformanceはまだ行っていない。

## REDからGREENへ

[原ログ・各段階のsource snapshot・SHA-256](../../benchmarks/results/task-bridge-2026-10-06/README.md)を保存した。弱い候補の試験用分岐は最終sourceへ残していない。

| 契約 | 先行RED | 最終確認 |
|---|---|---|
| bridge APIを実際に使う | API未定義E0432、exit 101 | private native 16群の検査へ接続 |
| 結果通知は終了確認ではない | sender通知だけでreceiveを返す候補がcleanup gate中に完了し、native 1件失敗 | actual joinまでreceive Pending、実join後に結果受取 |
| 受取後もscope故障を残す | receiveでprimaryを消す候補が出口Okとなり、native 1件失敗 | receiveのErrを受け取った後も再join/出口Err |
| 元legacy Errorを保持する | getter未定義E0599。その後None getterのnative 1件失敗 | Invalid primary・Busy関連causeのkind/messageをclone・再join後も保持 |
| checker診断の元行を確認する | runnerの相対pathとcanonical path不一致で元行None | 入力pathを共通canonical化。High/Low既存checker負例で元行を確認 |

最後の問題はcompiler本体ではなく新runnerにあった。修正中、手書きLow fixtureのコメントを`//`と書いたためparseで失敗した記録も残し、lexerが対応する`#`へ直した。parse失敗を負例の成功へ読み替えていない。

最終private runtime 16群は次を確認する。

- 異種T、内側の業務Result::Err、健康な兄弟の継続。
- sender Readyとchild Future cleanup gateを分け、actual joinより先にreceiveが完了しないこと。
- fault観測→兄弟cancel要求→全task実join→外側Err。受取後のsticky failure、後続関連cause、body元Err優先。
- receiveの未poll/Pending Drop、drain途中取消後の再開。discardはabort/detach/join省略ではないこと。
- 要求済みCancelledと予期しない取消、要求後に実際に返ったpanic/legacy Errの区別。
- native ID再利用をfake seamで固定し、旧未受取ticketと新ticketを混同しないこと。
- 誤ったscope owner、成功join後の出力欠落をprotocol faultとして扱い、他Scopeのjoinを奪わないこと。
- parent Dropはabort要求まで。送信失敗T、buffer T、受取TのDrop場所を分ける。故障後spawnも親の入力評価・実登録・実join責任を保つ。

順序はoneshot/Notify/独立eventとDrop gateで作る。10秒のwatchdogは試験のhang検出用で、runtimeの新しい期限ではない。子Drop内でwatchdog assertをpanicさせず、gate解放と可能なcleanupの後、親側で失敗を報告する。

## 確認した結果

| 検査 | 結果と範囲 |
|---|---|
| private bridge native | 16成功。Rust内のprivate prototypeをTokioで実行 |
| runtime全lib | network権限付き206成功。初回のsocket PermissionDeniedは別logへ保存 |
| 先行入力の登録検査 | 56入力の登録・High/Low対・元行markerを確認。意味論の成功ではない |
| runner oracle | 3成功。拒否段階、診断/元行不一致、実checkerの元位置を確認 |
| 将来Task契約の現compiler照合 | 6一致・50未達、exit 1。既存互換4入力＋既存checker負例2入力のみ一致。新Task 50入力は未実装のparse RED |
| fmt / runtime all-targets clippy | 成功 |
| workspace全回帰 | 成功。ログの93 result block・919成功（うち子プロセスのimage_child再実行3件を含む）、failed/ignored 0。debug symbolsなし、build jobs 2、test並列度は既定 |
| workspace fmt / all-targets clippy・runner oracle再確認 | 成功 |
| source head `6223ad2`のCI | [checks 37467579939](https://github.com/disnana/Nagi/actions/runs/37467579939)と[website 37467579497](https://github.com/disnana/Nagi/actions/runs/37467579497)成功。4 OSでprivate16群が実行され、各16成功。Ready to merge成功、release skip |
| allocation / Future size / throughput | 未測定。今回の一区切りには含めない |

## 設計上の問題と限界

新たなpublic P0/P1反例は今回発見していない。失敗した候補はprivate試作のP1相当であり、結果通知をjoinと混同すること、receiptでscopeの故障を消すこと、表示文字列だけで元Errorを置き換えることが共通原因だった。runnerの元行不一致はP2。これらを先行oracleで固定し、最終試作では解消した。

次の未実装・未検証項目は残る。

| 項目 | 種類・次の行動 |
|---|---|
| 全Tの正常出口await/discard、moveと義務の移動、非shared、scope escape拒否 | S1必須の未実装契約。Rustの型owner tokenだけでは保証しない。checkerのbinding/ScopeId/consume factsとnegative testsへ接続する |
| sealed Spawn/Await/Discard plan、保存Low、Rust生成、public Error変換 | S1必須の未実装接続。emitterでownershipを再推論せず、High・保存Low・手書きLowのnative conformanceで確認する |
| body locals退役→abort/drainのcompiler接続 | runtimeのbody元Err保持だけではNagi localsの順序を保証しない。既存cleanup anchorと生成planを検査する |
| entry退役と保持量 | P2候補。private試作は次のScope操作で全entryをretain/sweepし、HashMap capacityや関連cause Vecも残る。長いbodyの完了未join、join済み未受取Tを含めて測定する |
| channel/receipt/causeのallocation、Fault message clone、Future size | 未測定。結果TのCloneは要求しないが、Arc/refcount/文字列のコストまでゼロとは言えない。公開接続前後を同条件で測る |
| Supervisor terminal→HTTP停止の移行 | S2後続。旧statement spawnを維持し、業務reply Errとservice故障を分類してから専用実socket oracleへ接続する |
| 公開SQLite Pool/Transaction・capacity | 別工程。今回着手しておらず、既存ブロッカーを解消したとは扱わない |

parent Future Drop/unwindの同期Dropはactual join完了を返せない。non-yielding処理の強制停止、副作用rollback、任意Rust Drop/panic payload Dropからの普遍回復は保証しない。prototypeのRust Taskはmoveにより一回consumeされるが、Nagiの全T未消費義務やscope外禁止を実証したわけではない。

## Solレビューと次工程

Solがchecker周辺・runtime bridge・runnerを独立読取レビューした。Task checker未実装とnative prototypeを分け、元Errorの消失と元行runnerの不一致を指摘した。修正後のsourceを再レビューし、新たなブロッカーは見つからなかった。レビュー担当はCargoを実行しておらず、上の実行ログをAIの成功報告で代用していない。

最初の一区切りは「先行RED＋private bridge検証」とする。その後の追加指示により、文書化と最新head・4 OS CIが成功したら、checker/ownership/scope義務、Low/Rust生成、native conformance、全回帰、測定、日英Docs、Solレビューまで継続する。旧spawnは一括置換せず、main向けPRの完成で止める。merge・release・version更新・SQLiteは行わない。


## ローカル検証環境の補正

workspace全回帰の初回はdebug symbol付きのCargo cache、二回目はworktreeのnative cacheがそれぞれ容量不足で失敗した。コンパイラ契約の失敗と混同せず原ログを保存し、再生成可能なcacheのみを整理した。debug symbolを省き、build jobsを2にし、native cacheを空き容量のある`/tmp/nagi-task-native-target`へ指定して全回帰を継続する。testの期待値・test並列度・公開ビルド先の契約は変えない。

最初のCI設定はrun値末尾の`tests::`を引用しておらずYAML parseで失敗した。コマンド全体を引用して構文検査を通した。CI開始前の設定エラーを4 OS成功や実行済みtestへ数えない。


## 最新の停止指示

ユーザーが「次の自然な区切りまで進め、Sol 6.1向け引継ぎを書いて停止」へ方針を変更した。上の継続指示はそれ以前の履歴である。今回はStage 1の文書・全回帰・CI確認を区切りとし、compiler/public runtimeのTask実装は開始しない。[引継ぎ書](handoffs/2026-10-06-task-bridge-stage1.md)に再開手順と未実装の契約をまとめた。以後の新実装は再開指示を受けてから進める。


## CIと最終の停止地点

source head `6223ad222016962328f9b0c2bedd0b90e36b38e5`のCIを完了確認した。Linux、Windows x64、macOS Intel、macOS Apple Siliconのprivate bridge stepはそれぞれ16成功・failed/ignored 0。各package job、Linux全検査、IDE二製品、website、merge gateが成功し、releaseはskip。4 OSの原ログ抜粋とjob ledgerをartifactへ保存した。

この検証済みsourceへ引継ぎ・結果・artifactを追記して停止する。Task checker/ownership/scope義務、Low/Rust生成、公開runtime、測定、SQLiteは開始していない。引継ぎ追記の最新head CIはPR Checksで別に確認し、上の実行済みsourceとproduction source hashが一致することを照合する。
