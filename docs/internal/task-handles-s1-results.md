# Task結果handle S1接続の実行記録

2026-10-06。ユーザーの再開指示によるS1接続の記録。再開時のmainは `9ba4a104be6f65dba61eda0c7b1ef0f98c892cf7`、PR #88 headは `5c2c8282fbd9841cd3b0a2a78e9742d7ec3a811e`。再開時のchecks `37471066148`、website `37471065829` は成功し、レビュー提出・threadは0件だった。接続前sourceはStage 1検証head `6223ad2`と一致し、入力62件・artifact122件のSHA-256を確認した。今回の接続sourceとCIは末尾へ分けて記録する。

ADR 012と接続判断を用い、追加RED、compiler/checker、公開runtime、Low/Rust、native、回帰・測定・日英文書・独立レビューを実行した。Stage 1の16private oracleや6/56一致を今回の成功へ数えない。merge、release、version更新、S2の具体API、公開SQLiteは対象外。

原ログと実装前snapshotは `benchmarks/results/task-handles-s1-2026-10-06/` に保存する。新しい環境にはRust/Cargoとcacheがなかったため、通常toolchainを `/workspace/toolchains/` に導入した。環境固有pathをrepository既定へ固定しない。

## 接続した範囲

SpawnBind・正式std.task metadata・構文ScopeIdとbinding義務・sealed Scope/Spawn/Receive/Discard planを接続した。全Tの一回消費/正常出口義務、moveによる義務転送、escape拒否をcheckerで扱い、通常Low最終checkとraw public checkerも同じ意味へ揃えた。emitterの名前リストによる所有権再推論や暗黙cloneは加えていない。Task bindingを含む最寄りscopeのみTaskScopeを使い、nested scopeは独立。旧concurrent.rs、Supervisor/HTTP、Cargo依存・版は変更しない。

private prototypeをruntime/src/task.rsのpublic本体へ移し、二重実装を残していない。TaskFailureはopaque・非Clone・非shared、kindは四値Copy enum、messageはFailure-origin view、discardはunit。bodyラベル/locals cleanup anchor/元Error変換を維持し、業務Resultの入れ子、sticky primary、全actual join、body元Errを保つ。同期Dropはabort要求まで。

## ローカル実行

| 検査 | 観測 | 限界 |
|---|---|---|
| Task契約 | 90/90一致（45 High/Low対、38受理・52checker拒否）、元行/診断を照合 | parse/import拒否をnegative GREENにしない |
| Task native | 全19 positive対をHigh・保存Low・手書きLowで実build/run。独立barrierのlifecycle三構文、固定seed16経路×両flag×三構文も成功 | compile-onlyやstubではない。任意スケジューリングの証明ではない |
| runtime | public 17oracle、公開API3、doc9（Task negative4）成功 | Rust private consumeだけでNagiのownership保証に代用しない |
| workspace | cargo test --locked成功。原ログ95 result block・933成功・failed0・ignored1 | 子process image_child再実行3件を含む。ignoredは費用用1件で、別途明示実行成功 |
| fmt/clippy | fmt check、workspace all-targets clippy -D warnings成功 | 意味論の証明ではない |
| 探索 | 固定seed1000mutation：parse731、check192、checked Low emit77。bounded native16成功、panic0 | coverage-guided fuzzではない。mutation受理77はnative実行の件数ではない |
| 旧アプリ | verify_application_examples.py 10 projects/19 High・保存Low・手書きLow実行成功 | CLIはdebug build、生成appはrelease。旧spawnを機械置換しない |
| Docs/site | 92 HTMLページのlinks/anchors/assetsを検証。current Markdownのlocal file linksも検査 | 外部URLは取得せず、歴史的source snapshotsを現行Docsへ数えない |

初回全回帰は既存の非Task await診断fragment不一致で失敗した。従来の診断を維持して全回帰を再実行した。fmt追記後の整形不足、site依存未導入、サンプルverifierの既定release compiler path不在も保存/区別し、test期待の緩和やinfra失敗をGREENにしない。

## 独立レビューと縮小反例

runtime担当Solがcompilerを独立レビューし、別のSolがruntime/cleanupを独立レビューした。確認済みP1はFailure wrapper copy/share（check受理後Rust E0277）、ユーザー名spawnのalias/binary/field、関連メソッドpub use（Rust E0432）、raw checker APIのcanonical Task欠落。縮小入力をcorpusへ残し、段階別原ログと修正後再実行を保存した。封印factsの欠落・bridge/ScopeId/action不一致はCompilerDefectとして拒否する。

runtimeの故障後大量receiveにはO(n²)の全record掃除が残っていた。entries128!=127のnative REDから、残joinがあればdrainし、受取対象ticketだけを退役する経路へ修正した。同じ公開APIの独立release harnessで8192件中央値97.3ms→1.75ms、17oracleと追加公開4組を再確認。一般的なlatency保証へ広げない。レビューは全経路の証明ではなく、原ログ・reviewed hashesは保存artifactにある。レビュー後にhashが変わったcheckerとruntimeはcargo fmtだけの差で、`reviews/final-format-provenance.json`と二つのdiffに対応を保存した。

## 同条件の生成Rustと手書きRust

Linux x86_64、rustc 1.99.0、current-thread Tokio、release。双方が同じTaskScope、入力、body Err/cleanup/join構造を使う。bare Tokioは比較していない。順次spawn/receiveと、一括spawn/discardを分け、25 loops×7反復、呼出しthreadのallocation counterを使う。全childが同じthreadで動くfixtureである。

| 項目 | Nagi生成Rust | 手書きRust |
|---|---:|---:|
| receive/discard Future | 416/408 B | 416/408 B |
| 8192 receive allocation / bytes | 32773 / 3342740 | 32773 / 3342740 |
| 8192 discard allocation / bytes | 32809 / 5538236 | 32809 / 5538236 |
| 8192 receive中央値 | 13.84ms | 10.84ms |
| 8192 discard中央値 | 13.33ms | 12.21ms |
| binary | 1254848 B | 1234304 B |
| package再build中央値（依存warm） | 2.87s | 3.40s |

時間は共有host、逐次測定順、cacheを含む短いサンプルでばらつく。allocation一致はこのfixtureの観測であり、ゼロコスト・普遍的な速度優位・clean buildを主張しない。全raw samplesと生成Rustをcost/へ保存した。再現準備は `cargo run --locked -p nagic --example task-handles-cost -- /tmp/new-output-directory`。

通常buildのScope248B、Task[u64]40B、Failure40B、Kind1B、receive/join/cancel Future128/96/112B。runtimeの一括受取Future488Bは言語の順次fixtureと別workload。cfg(test) fake ID layoutのallocation/速度は通常buildと混同しない。内部保持観測では1024×64B payloadが完了未joinとjoin済み未受取で残り、handle Dropでpayloadが消え、次のscope sweepでrecordが消える。map容量、primary/related127 causeは保持し得る。取消要求と停止確認、受取放棄とclose完了を分ける。

## 公開確認

接続sourceを `34ac4d585084877372965e3d58ed5c2002604529`、tree `781e031eafbdc0efa00038007d41cf16dc538862` として公開し、PR #88の実headとtreeを読み戻した。CLI pushはGitHub書込み認証がなく失敗したため、接続GitHub APIでtree/commit/refを作成した。ローカルcommitとはSHAが異なるが、treeと151 source hashesは一致する。

このsourceの[checks run 37489343115](https://github.com/disnana/Nagi/actions/runs/37489343115)と[website run 37489342523](https://github.com/disnana/Nagi/actions/runs/37489342523)は成功した。各jobの原ログ、run/job metadataと集計はartifactの `ci/source-34ac4d5/` に保存した。

| target | Task native | runtime oracle | 公開API / doc | checker契約 |
|---|---:|---:|---:|---:|
| Linux x86_64 | 5 | 17 | 3 / 9 | 90/90 |
| Windows x86_64 | 5 | 17 | 3 / 9 | 90/90 |
| macOS ARM64 | 5 | 17 | 3 / 9 | 90/90 |
| macOS Intel | 5 | 17 | 3 / 9 | 90/90 |

すべてfailed/ignored 0。nativeの5群には全19 positive対の三構文と生成探索を含み、checker契約90件とnative実行件数は別である。フィルタによる0件実行を成功に数えていない。Linux全workspace原ログも95 result block・933成功・failed0・費用用ignored1で、fmt/clippy、no-default-features、実HTTP、fuzz、全package・両IDEとmerge gateが成功。websiteは92ページのbuild/link検査成功、PRなのでdeployはskip、release publishもskipだった。

後続の結果・CI追記は文書/artifactのみで、production sourceの151 hashesを維持する。追記headの必須ChecksはPR #88で別に読み戻し、上のsource headの実行と分ける。Stage 1のCIを今回の実行として数えない。main向けdraftを維持し、merge、release、version更新、S2の具体API、公開SQLiteは行わない。
