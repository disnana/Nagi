# Docs onboarding audit

PR handoff and evidence for this audit are in the [internal onboarding handoff](handoffs/2026-10-08-docs-onboarding.md) and [verification bundle](../../benchmarks/results/docs-onboarding-2026-10-08/README.md).

基準日は2026-10-08、確認対象のsource treeは`676576724829e45b077b58628bfe2417e6cf3673`を親とするDocs作業branch。公開対象ページの棚卸し、日英route parity、初心者が最初のアプリと本体変更に到達する導線を記録する。compiler/runtimeの仕様全体を再監査する文書ではない。

## 方法と範囲

`website/build.py`の日本語・英語source一覧を照合し、公開Docs、root README/CONTRIBUTING、website home/navigation、主要言語referenceを読み、見出し・リンク先とチュートリアル経路を確認した。`P`は本文の目的・章構成を確認、`D`は初学者が該当機能を使う説明まで読み合わせ、`W`はwebsiteの生成routeと内部link/anchor検査、`X`は記載したsourceをcompilerでcheckまたはrunした範囲を表す。各行に記録した範囲を超える全API意味論、外部URL到達性、Windows/macOS上の操作を検証したという意味ではない。

変更した導線は、compiler導入から1 file CLI、手動境界テスト、エラー確認までを[最初のCLIアプリ](../first-app.md)へ、本体の環境準備、repository map、検索、回帰fixture追加、test、diff、commit、fork remote、main向けPRまでを[初めての貢献](../contributing.md)へ接続した。どちらも日英に対応し、root README/CONTRIBUTINGとhome・site navigationから入れる。

公開compiler Nagi 0.1.11で明示moveとTask結果handleが公開済み、JetBrains plugin 0.1.1は独立componentで公開済みとして表記する。website navigationに残っていたTaskの「未リリース」labelを除去した。SQLiteはこの作業の変更範囲外であり、`docs/database.md`、`docs/en/database.md`およびSQLite internal資料を編集・意味監査していない。ここでは現行branchの旧Db APIだけを指し、Pool/Txを公開済み・利用可能とは説明しない。

## ページ別の既存説明・ギャップ・確認範囲

各JP/ENペアはwebsiteに登録された同じslugである。`W`は内部route/link検査、`P`は章・用途の確認、`D`は内容の重点照合、`X`は限定した実行またはcompiler checkである。

### 入口と言語reference

| JP / EN page | 既存説明と重複・不足 | 今回の確認 |
|---|---|---|
| [README](../README.md) / [README](../../docs/en/README.md) | セットアップとreference入口はあるが、1 file実用アプリと本体への小変更経路がなかった。言語機能の見出しだけでは例の結果・誤例・選択肢が辿りにくかったため全機能索引を追加。 | D, W。機能ごとの用途/例/制約/誤り・修正/使い分けの所在を照合。 |
| [getting-started](../getting-started.md) / [getting-started](../../docs/en/getting-started.md) | install、Hello World、check/buildの切り分け、VS Code、updateがある。導入後の実用入力・test・修正は扱わず、新CLI教程へ分担。 | P, W。各OSのinstaller操作は未実行。 |
| [language-guide](../language-guide.md) / [language-guide](../../docs/en/language-guide.md) | 値、関数、List/class/control、view、Result、moduleの順で学ぶ。実用アプリの単独手順とtest/practice flowは別ページへ分離。 | D, W。first-appへの用語リンクを確認。全章のsampleは再実行していない。 |
| [first-app](../first-app.md) / [first-app](../../docs/en/first-app.md) | 新設。budget判定CLIでstdin、型付き失敗、境界値、修正をつなぐ。 | D, W, X。0.1.11 compilerでJP/EN掲載code一致、Highとstandalone saved Lowのcheck/build、各700/1000/1001・`abc`・`-1`をnative実行（計10件）まで確認。さらにscratchで`<=`を一時的に`<`へ変え、1000が`over budget`になることを観測後、`<=`へ戻して3境界値を再確認。実行記録は指定log folder。 |
| [editor](../editor.md) / [editor](../../docs/en/editor.md) | VS Codeのinstall/補完/移動機能を説明。IDE連携からcompiler開発環境への移行は新contribution pageへ。 | P, W。VS Code/JetBrains上のUI操作は未実行。 |
| [syntax](../syntax.md) / [syntax](../../docs/en/syntax.md) | ファイル、値、関数、control、operator、class、Result、async、importの早見表。詳しい用途はreferenceへ続く。 | D, W。文法項目から機能索引先へ辿れることを確認。全例compileは未実行。 |
| [builtins](../builtins.md) / [builtins](../../docs/en/builtins.md) | I/O、文字列/List、変換、JSON/HTML、async/HTTP/SQLite、shared、測定関数を署名と条件で検索。 | P, W。関数ごとの全実行値は再検証していない。 |
| [types](../types.md) / [types](../../docs/en/types.md) | enum、標準resource、function value、未対応操作、Rust representationを説明。基本的な変数の値と再代入はguide/syntaxに分担。 | D, W。型索引との整合を確認。全型matrixは未再実行。 |
| [classes](../classes.md) / [classes](../../docs/en/classes.md) | named fieldとclassの対応範囲を説明。誤ったconstructorの名前/位置引数、enumとclassの選択補助を追加。 | D, W。正誤例と翻訳を確認。各例のnative buildは未実行。 |
| [ownership](../ownership.md) / [ownership](../../docs/en/ownership.md) | move、明示move代入、copy/shared、field、loop、viewの範囲が詳しい。最初のuse-after-moveの誤例とview/copy/ownership transferの修正を追加。source-level transferをzero-cost保証と読める表現を改めた。 | D, W, X。誤ったmove後使用が`check`で拒否、移譲先を使う形が受理。性能・native costは測定していない。 |
| [view-and-zero-copy](../view-and-zero-copy.md) / [view-and-zero-copy](../../docs/en/view-and-zero-copy.md) | view、文字列範囲、copyと既存受渡しを詳述。ownership pageは一般的な移動/借用規則を担当する。 | P, W。zero-copy保証を新たに追加していない。各sample実行は未実施。 |
| [error-handling](../error-handling.md) / [error-handling](../../docs/en/error-handling.md) | Result/Option/Err/panic、match、独自errorを説明。nullableとplain return内の`try`の誤用を対比例にし、結果・失敗伝播の使い分けを追加。 | D, W, X。nullable/Result誤例は拒否、修正版はcheck受理。first-appの入力失敗を実行時エラーとして確認（compiler診断とは区別）。 |
| [low-language](../low-language.md) / [low-language](../../docs/en/low-language.md) | High→Low変換、Low直書き、関数差し替えを説明し、同じ型/ownership規則の範囲を示す。 | P, W。Low専用実行matrixは未実行。 |

### アプリ、async、runtime/API reference

| JP / EN page | 既存説明と重複・不足 | 今回の確認 |
|---|---|---|
| [modules-and-rust](../modules-and-rust.md) / [modules-and-rust](../../docs/en/modules-and-rust.md) | 相対file import、module alias、標準library、Rust adapterを説明。multi-file beginner applicationはlanguage-guideへ、repositoryのRust code navigationはcontributingへ分離。 | P, W。Rust exampleはbuildしていない。 |
| [libraries](../libraries.md) / [libraries](../../docs/en/libraries.md) | Nagi共通code、Rust crate、High/Lowのlibrary境界と今後の整備を説明。 | P, W。library build未実行。 |
| [projects](../projects.md) / [projects](../../docs/en/projects.md) | `nagi.toml`、entry file、local crate、CLI precedence、symbolsを説明。 | P, W。CLI precedence実行は未検証。 |
| [async](../async.md) / [async](../../docs/en/async.md) | `await`/scope/legacy spawnとTask入口を説明。Futureを変数へ保存する誤例と直接awaitする修正を追加し、Task契約は別referenceへ寄せる。 | D, W, X。Future保存はcheck拒否、直接awaitは受理。async native runtime/取消の新しい実験は未実行。 |
| [task-handles](../task-handles.md) / [task-handles](../../docs/en/task-handles.md) | Taskの一度消費、正常出口義務、外側TaskFailureと内側business Result、Supervisor移行を説明。未await Taskとnested Resultの誤解/正しいmatchを追加。 | D, W, X。未処理Taskはcheck拒否、await/discardとnested Result例はcheck受理。runtime semanticsをこのcheckだけで保証しない。 |
| [concurrency](../concurrency.md) / [concurrency](../../docs/en/concurrency.md) | CPU処理とasyncの違い、データ受け渡し/現在の制限を説明。async overviewとTask lifecycleの詳細を重複させない。 | P, W。CPU stress/performance test未実行。 |
| [actor](../actor.md) / [actor](../../docs/en/actor.md) | message/state/reply、capacity、call errorを説明。handlerの業務Errとouter worker Errの正誤対を追加。 | D, W, X。二つのhandler形をcheck受理。Actor runtime・restart結果は未実行。 |
| [supervisor](../supervisor.md) / [supervisor](../../docs/en/supervisor.md) | restart policy、shutdown、monitor、scopeとの接続を説明。`PERMANENT`がnormal completionでも再起動する具体的な誤選択を追記。 | D, W。runtime restart/shutdown実行は未実施。 |
| [actor-reference](../actor-reference.md) / [actor-reference](../../docs/en/actor-reference.md) | actor/Supervisor API signatureと型一覧。導入・選び方はactor/supervisor guideへ分担。 | P, W。API matrixを全件再実行していない。 |
| [actor-performance](../actor-performance.md) / [actor-performance](../../docs/en/actor-performance.md) | 固定sample・測定条件・性能範囲を案内。一般API保証とは区別。 | P, W。benchmark未実行。 |
| [http](../http.md) / [http](../../docs/en/http.md) | 最小server、request/response、custom error responseを説明しAPI referenceへ送る。 | P, W。HTTP実server未実行。 |
| [http-server](../http-server.md) / [http-server](../../docs/en/http-server.md) | server/route API詳細を案内。使用手順の文章はhttp guideと役割を分ける。 | P, W。API意味論の再監査・server test未実施。 |
| [http-stdlib-performance](../http-stdlib-performance.md) / [http-stdlib-performance](../../docs/en/http-stdlib-performance.md) | HTTP標準libraryの性能測定条件を案内。 | P, W。測定未実行。 |
| [http-legacy](../http-legacy.md) / [http-legacy](../../docs/en/http-legacy.md) | 旧HTTP API/移行境界を記録。新HTTP guide/referenceと重複する範囲は移行用に限定。 | P, W。旧API実行未実施。 |
| [json](../json.md) / [json](../../docs/en/json.md) | 型/borrow、入力validationを説明。複雑な例はsample projectsへつなぐ。 | P, W。JSON runtime未実行。 |
| [database](../database.md) / [database](../../docs/en/database.md) | SQLite guide。親branchが担当するため内容は閲覧/編集対象外。このbranchからは現行旧Db APIを参照し、Pool/Txを案内しない。 | Wのみ（サイトsource/link生成）。SQL例の意味検証なし。 |
| [sql-check](../sql-check.md) / [sql-check](../../docs/en/sql-check.md) | Offline DDL snapshotに対するliteral SQL preflight。通常runtime queryのtutorialではない。 | P, W。DB/compiler SQL test未実行。 |

### Sample、設計、開発、サイト

| JP / EN page | 既存説明と重複・不足 | 今回の確認 |
|---|---|---|
| [library-examples](../library-examples.md) / [library-examples](../../docs/en/library-examples.md) | 複数featureを合わせた実行projectの索引。CLI first-appより大きい実例を担う。 | P, W。リンク対象sampleは実行していない。 |
| [web-demo](../../test-nagi-code/web-demo/README.md) / [web-demo](../en/web-demo.md) | Web demo実行手順とproject説明。最初のlanguage tutorialには範囲が広いため使用しない。英語の公開sourceは`docs/en/web-demo.md`。 | P, W。demo実行なし。 |
| [result-api](../../test-nagi-code/result-api/README.md) / [result-api](../en/result-api.md) | Resultを使うAPI sample。error-handling guideの小さなoffline例より実務寄り。英語の公開sourceは`docs/en/result-api.md`。 | P, W。service build/runなし。 |
| [library-design](../library-design.md) / [library-design](../../docs/en/library-design.md) | 未実装を含むlibrary design proposalを公開APIと分離。 | P, W。提案の再決定なし。 |
| [introduction](../introduction.md) / [introduction](../../docs/en/introduction.md) | 目的、範囲、現状と制限の紹介。詳しい言語説明を重複しない。 | P, W。記載全featureの再検証なし。 |
| [memory-model](../memory-model.md) / [memory-model](../../docs/en/memory-model.md) | ownership/runtime memory modelを深掘り。初学者はownership guideから進む。 | P, W。memory profile/Drop matrix未実行。 |
| [compiler-internals](../compiler-internals.md) / [compiler-internals](../../docs/en/compiler-internals.md) | pipelineとHigh/Low/Rustの境界を説明。 | P, W。architecture code changeなし。 |
| [code-map](../code-map.md) / [code-map](../../docs/en/code-map.md) | `nagic map`でNagi applicationを図示する機能。repository自身のRust source mapと誤解しない注意をcontributingに追加。 | D, W。実map生成は未実行。 |
| [queue](../queue.md) / [queue](../../docs/en/queue.md) | 検証用queue API。一般的なactor推奨と混同しない。 | P, W。queue runtime未実行。 |
| [ffi](../ffi.md) / [ffi](../../docs/en/ffi.md) | Rust/他言語との境界と未対応範囲。通常のNagi APIと区別。 | P, W。外部adapter build未実行。 |
| [performance](../performance.md) / [performance](../../docs/en/performance.md) | 測定の読み方と限定条件。ここではcost/zero-cost保証を追加しない。 | P, W。性能測定未実施。 |
| [measurements](../../PERFORMANCE.md) / [measurements](../../PERFORMANCE.md) | repository内測定結果。条件・制約を解説するperformance pageと分担。 | Wのみ。数値とartifactは再計算していない。 |
| [http-capacity](../http-capacity.md) / [http-capacity](../../docs/en/http-capacity.md) | HTTP負荷試験の条件と結果。| P, W。load test未実行。 |
| [roadmap](../roadmap.md) / [roadmap](../../docs/en/roadmap.md) | 未実装・予定の記録。Docs feature indexは実装済みAPIと設計予定を区別。 | P, W。roadmap項目を再決定していない。 |
| [vscode-extension](../../editors/vscode-nagi/README.md) / [vscode-extension](../en/vscode-extension.md) | VS Code extension install/development guide。英語の公開sourceは`docs/en/vscode-extension.md`、compiler versionとは別component。 | P, W。IDE install/test未実施。 |
| [root README](../../README.md) / [root README.en](../../README.en.md) | download/quickstartからCLI・本体貢献tutorialを辿れる入口を追加。READMEとDocs indexの内容を重複させず、詳細はDocsへ送る。 | D。linksはWで確認。 |
| [root CONTRIBUTING](../../CONTRIBUTING.md) / [root CONTRIBUTING.en](../../CONTRIBUTING.en.md) | 方針、報告、PR基準、検証policyを保持し、新実践guideへ送る。 | D。internal policy/PRを投稿していない。 |
| [website home](../../website/templates/home.html) / [home EN](../../website/templates/home.en.html) | heroの入門経路とstatus sectionの本体貢献入口を追加。homeは短い案内、Docs tutorialが具体操作を担当。 | D, W。HTML生成のみ。browser/visual accessibility testなし。 |
| [website nav](../../website/build.py) | 日英同一page setを登録。first-app/contributingのnav導線とTask release labelを修正。 | D, W。出力routeと内部link/anchorをbuild検査。外部link・browser操作なし。 |
| [website README](../../website/README.md) / [website README EN](../../website/README.en.md) | build/preview、GitHub Pages、編集対象と説明方針を日英で説明。以前英語contributionから日本語手順へ送っていたためEN counterpartを追加。 | P, W。site source linkが解決。website site自身のnavigationには載せない。 |

`docs/internal/docs-onboarding-audit.md`自体はwebsite navigationに公開していない。上記の`database`と他のSQLite internal資料は親branchの担当範囲であり、source linkの生成確認以上の監査結果を含まない。古い作業記録内の「未リリース」は記録時点の履歴として残る場合がある。現在の公開案内に使うrelease statusは、公開Docsとsite navigationで確認する。

## 言語機能 coverage の所在

索引の各列から以下の情報へ辿れることを確認した。機能ごとの正しい動作はcompiler/runtimeの独立contract確認を代替しない。

| 機能群 | 用途・動く例/結果 | 制約・誤り/直し方 | 選択肢・詳細 |
|---|---|---|---|
| 値、型、代入/reassignment | language guide §1 | 型推論後の違う型への再代入を避ける | types, syntax |
| 関数、function value、return | language guide §2 | parameter/return typeと`main`呼び出し | syntax, types |
| control/operator/List/class/enum | language guide §3、classes | bool condition、unsupported loop control、named field constructor | syntax, types, classes |
| nullable `T?`/Option | error-handling Some/None example | direct `value + 1`拒否、全case matchへ修正 | types, syntax |
| Result/try/match/Error | first-app、language guide §5、result.nagi | plain return内try拒否、Err伝播とlocal match回復 | error-handling, builtins |
| ownership/move/copy/shared | language guide §4、ownership | use-after-move、move/copy/viewの修正とコスト保証の境界 | ownership, memory-model |
| borrow/view | view example | borrow escape、owner再利用制限、temporary view拒否 | ownership, view-and-zero-copy |
| modules/Rust interop | language guide §6 | relative paths/module cycle/package-search limits | modules-and-rust, ffi |
| async/await/Task/scope | async examples、Task page | Future storage、未受取Task、nested business Errとouter fault | async, task-handles, concurrency |
| Actor/Supervisor | supervised-service sample | handler business Err placement、restart policy wrong choice | actor, supervisor, actor-reference |
| High/Low/built-ins | High/Low example、builtins reference | High/Low共通の型/ownership、per-function conditions | low-language, builtins, modules-and-rust |

## 実行・サイト検査の範囲

実行ログを`/workspace/nagi-docs-onboarding-2026-10-08/`に保存した。Nagi compiler 0.1.11は現在のDocs worktreeから`cargo build --locked -p nagic`してcheck/runに用いた。build cacheを使ったためclean buildとは記録しない。教程の初回runはネットワーク制限でcrate index取得に失敗し、Cargo offline modeと既存cacheで再実行すると成功した。利用者向け本文はcache固有pathを使わず、dependency取得が失敗する場合の確認先を案内する。

`examples/tutorial/first_app.nagi`とJP/EN page codeの完全一致を確認し、dedicated working folderでHighとstandalone saved Lowをそれぞれcheck/buildした後、700/1000/1001、`abc`、`-1`をnative実行し計10件の結果とexit codeを記録した。加えてscratchで境界比較を一時変更し、1000の誤判定と修正後の3結果を確認した。Contributingのfixture例は別scratch worktreeに`nullable_default.nagi`とcorpus entryを追加してsource checkし、`cargo test --locked -p nagic --test conformance corpus_and_bounded_generated_contracts_reach_native_execution -- --exact`、`git diff --cached --check`、staged diffを検証した。fork作成、push、commit、PR作成は外部GitHub操作であり、このDocs作業では行っていない。記載したWindows PowerShell/macOSのコマンドもこのLinux environmentでは未実行。

所有権、nullable、Result、Future storage、Task、nested Result、Actor handlerの新しい例は0.1.11 compilerでcheckした。intentional invalid snippetsは期待どおり拒否され、修正版は受理された。これらはruntime behavior、全High/Low variants、Rust adapter、HTTP/Actor lifecycleを検証したものではない。

websiteのHTML previewはPython 3.12と既存site venvで生成し、日英のsite route、内部linkと見出しanchorをbuild時に検査する。tutorial drift／High/Low検証は[10件のnative結果](../../benchmarks/results/docs-onboarding-2026-10-08/native10-results.json)と[harness log](../../benchmarks/results/docs-onboarding-2026-10-08/logs/onboarding-high-saved-low-success.log)、境界比較の一時変更と復元確認は[boundary log](../../benchmarks/results/docs-onboarding-2026-10-08/logs/boundary-regression-review.log)に記録した。最初のharness呼び出しはtemp directoryに`nagi.toml`がないままproject modeで`lower`して失敗したため、standalone fileとして再実行して成功した。失敗と成功の記録はそれぞれ[initial log](../../benchmarks/results/docs-onboarding-2026-10-08/logs/onboarding-project-mode-failure.log)、[success log](../../benchmarks/results/docs-onboarding-2026-10-08/logs/onboarding-high-saved-low-success.log)にある。compiler/native buildは既存cacheを利用しておりclean buildではない。外部URL到達性、ページの視覚表示、IDE上の操作は検査対象外。全ての既存API例をrunしたという記録ではない。
