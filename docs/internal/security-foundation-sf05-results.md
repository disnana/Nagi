# SF05 実装と検証結果

2026-10-09。base main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`、作業branch `feat/security-foundation-sf05`。SF00 #100 / SF01 #101の採用設計が前提。[契約](security-foundation/sf05-contract.md)・[ADR 014](adr/014-literal-query-and-sqlite-admission.md)・[移行inventory日英](security-foundation/sf05-migration-inventory.md)を参照。source/Docsの独立reviewは下記の再確認で完了し、Draft PR #106へ送信済み。最新修正headの4 OS CIは未完了。merge/版/tag/正式releaseは実行していない。

## PR #106 release-plan mock の追加記録（2026-10-10 UTC）

PR #106はDraftのままで、公開headは `5e992b6cd775912b6886c0b6763a4364642c5132` / tree `17a53331a7fb31532042f0c01419134ed788ef83`。run #525の `release-plan` はPython suite 116件中3件で失敗した。3件とも `SQL checked 1 literal queries; 0 runtime/unsupported sites` というmock応答を受理したことが直接の失敗原因で、`changes` は成功。2026-10-10 06:46:23 UTCのjob readbackではLinux jobが `cargo test --locked` 中、3つのpackage jobはskipだった。これは公開PR headの実行結果である。

公開headに対する追加差分は `scripts/releases/test_verify.py` だけのtest-mock補正と、その結果・原ログの保存である。固定行番号を仮定せずsource内の `sqlite.literal(` 行とその行番号を見つけ、gate fixtureの二つの匿名 `?` と一つの `bind_i64` を認識する。これはこのfixture向けのmockであり、一般SQL parserではない。17個すべての `test_*` method ASTは元sourceと同一で、`scripts/releases/verify.py` は変更されていない。保存済みのlocal REDは17件中14成功・3失敗、補正後GREENは17成功・0失敗・0 ignored。元のlocalログに実行コマンド記録はない。別の新しい確認実行ではargv・cwd・時刻・exit・source/raw hashを保存し、17件成功を確認した。元の記録不足を後から埋めたことにはしない。

この追記時点では補正を含む公開CI runはない。元の公開runの失敗は解消扱いにしない。2026-10-10 07:13 UTCのreadbackで元headのLinux jobは成功、package/IDE jobsはskip、Ready gateは失敗だった。[test-only mock artifact](../../benchmarks/results/security-sf05-distribution-mock-2026-10-10/README.md) に元/補正後source、patch、local raw logs、公開job raw logと当時のreadbackを保存した。[独立reviewと別の記録付き確認](../../benchmarks/results/security-sf05-distribution-mock-review-2026-10-10/README.md) は原packetを変更せず保存し、必須指摘なし。これらのmock検査を実compiler・SQL engine・native配布検証の成功へ数えない。

## 変更と保証境界

canonical std.db.sqlite.literalの直接literal constructorとopaque Copy Queryをsealed planへ接続した。標準query/all/execはQuery必須。旧Db/db_*とruntime public Db/Sqlは実行経路ごと削除し、checkerだけに具体migration診断を残す。ユーザー同名定義はcanonical identityで区別する。通常checkにSQL parser/engine依存を増やさず、opt-inの既存SQLite engineがshape・必要列・直接builderのbind数をprepare-onlyで検査する。

trusted reviewed adapterは実Grant subject/targetを実Parametersへbindし、owner/id predicateが正しい対象だけを更新する。Tx予約は実bounded queueの容量だけ。local admissionはSF01と同じpermit発行で線形化し、同期callbackが一回enqueueした後のreplyだけをawaitする。発行前の失効は0enqueue、発行後は実enqueue前でも受理済み。有限容量待機中の失効、未使用予約Dropによる容量解放、enqueue後・SQL step前の失効も実SQLiteで観測する。

literal/Query自体はtenant認可の証明ではなく、任意Rust adapterの内部はtrusted境界。mutable permission/sessionは同Tx predicateか再確認を要する。SF02永続Session/世代、SF03 CSRF/CORS等は未完成。取消/後の失効は受理済み副作用のrollbackを保証しない。Bootstrapは固定レビュー済みstartup管理をrequest handlerから分離し、動的factoryを作らない。既存authorizerの許可/拒否は変更しない。

## 保存済み検証

原ログ・command/exit/toolchain/cache・入力SHA-256は[artifact root](../../benchmarks/results/security-sf05-validation-2026-10-09/)に保存。別worktreeのwarm targetを再利用しておりclean buildとは呼ばない。compiler/runtimeがこのSF05 worktreeのsource fingerprintで再compileされたログを保持する。

| 検査 | 現在の証拠 |
|---|---|
| 最小RED | 旧Db/db_open/string実行の3境界、High・High削除後保存Low・手書きLowの全9経路で旧受理を記録 |
| SF05 checker / 独立registry / SQL opt-in | 12 / 4 / 14件、目的診断と元source行。literal別名・動的入力・forge・ユーザー同名・全旧入口を含む |
| Native 三構文 | sqlite_public 8 + protected native 1。正対象affected1、別subject/target affected0、値101/200、実HTTP denial |
| Runtime追加差分 | final-admission: SQLite79 + public API1、失効/permit/enqueue/容量/decoder/authorizer/cleanup/actual close。0ignored |
| SQL engine無効構成 | engine-free: SF05 12 + registry4 + SQLdisabled1、通常checkを維持しopt-inのみfeature要求。default compiler再build済み |
| Workspace | final-full-offline: 1011成功・failed0・既存measurement1ignored、exit0。現時点の後差分はSQLite native testとconstructor拒否testの2ファイルのみ、production compiler/runtime hashは一致。追加nativeケースは上の79件で確認し、追加のliteral同士の連結／関数返却literalの三構文拒否はfinal-scopedのdefault/no-engine各12群で確認 |
| fmt / clippy / fuzz | quality: 各exit0。fuzz seed305419896、1000text mutations、77checked Low/emit mutations、16bounded native cases、panics0 |
| Node editor | 22files/196実行、failed0/skip0。compiler実体とsource hash付き。manual VS Code host GUIは未実行 |
| 配布外check | copied compilerを無関係cwdで実行、PATH空/runtime欠落で通常checkとSQL opt-inの良いquery/列typo/bind不一致を元行付きで検査 |
| application/tutorial | 10projects/19High-Low実行、typed nullable設定DB/restartを含む。日英SQLite tutorial一致、High・High削除後保存Lowのcheck/build/nativeで7/closedを観測 |
| HTTP業務 | CRUD/inventory/tasks/Result APIのHigh・元High削除Low、8native runs/84小さい業務checks。matching compiler binaryと全入力source hashを比較したcombined台帳。前半runの後続fixture失敗は成功へ変更せず明記 |
| Query費用 | release/x86_64 Linux、generated/manualとも未poll Future literal392B/selected408B。各mode4warmup＋32samples、calling-thread allocation全sample2回/280B、SQL構造string複製なし。rollback＋actual close成功、依存checksum/版はworkspace lockと一致。timeは共有hostの一回観測、thresholdなし |
| 日英Docs/website | source一致・相対リンク欠落0、最終集合とSHAはdocs-current-links.json。website102ページのlink/anchor/assets GREEN。改名済みアプリを含む上記native業務実行も完了 |

既存ignored task費用measurementは成功件数へ含めない。先行workspace runのserde_path_to_error取得タイムアウトはinfra failureの原ログとして保存し、成功に読み替えない。offline cacheで正常終了した別runを上の根拠とする。Node全suite初回のPATH未設定nagic ENOENT、sandbox EPERMと追加HTTP harnessの404 content type/TIME_WAIT/import closure/High削除処理の失敗も保存し、fixture目的GREENとして扱わない。後者は処理系変更で隠さずharnessの同じ原因をimport closureと明示したoriginal source集合で修正した。

## 完了前の残検査

source/Docsとtest-mock追加差分の独立reviewは完了し、最新修正head4 OS CIを残す。三構文最終native9群のsource/Rust/build/run原出力216filesは補完済み。結果を取得するまで完成・merge-readyとは報告しない。

実装者からの[引継ぎ](handoffs/2026-10-09-security-foundation-sf05.md)と[証跡索引](../../benchmarks/results/security-sf05-validation-2026-10-09/README.md)に、source一致の確認方法と未確認targetを記録する。


## 独立レビューの追記

local source head `c33e8c9`への独立Sol High reviewでは、実装上のblockerは未発見だった。Docsのpermit/enqueue順序P2と英語DESIGNの移行欠落P3を[台帳](security-foundation/review-log.md#sf05-実装の独立レビュー2026-10-09-utc)へ記録し、日英4文書を修正した。gate内は一回permit発行までで、gate解放後に同期callbackがenqueueし、replyをawaitする。permit後・enqueue前の失効も受理済みである。原snapshotの入力hashを後のDocsへ流用せず、後差分はreview-docs-correction.jsonへ別に保存する。production/test差分0なので同じ全回帰の再実行は追加せず、Docs再確認と最新head CIを行う。

845d938の独立Docs再確認でSF05-R01/R02を閉鎖し、source/Docsは承認相当。独立追加testのraw file保存はなく、実装者artifactとは分ける。最新headの4 OS CIは未確認。
