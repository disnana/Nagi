# JetBrains hover / Quick Documentation

2026-10-22追記: 利用者が最新候補の動作を確認し、0.1.4としての公開準備を依頼した。以下のGUI未確認・cold未対応の記述は各検証段階の履歴。現在の受入範囲と版更新gateは[0.1.4公開準備](jetbrains-0.1.4-release.md)を参照する。

2026-10-22。base main `765f0b0`。compiler/CIを変更せず、plugin側でcompiler事実の表示を追加する未リリースの作業。

以下の初期設計・検証記録と、その後の色保持/cold非同期修正を段階別に残す。現候補のcold動作は末尾の「cold要求の非同期完了とlocals」で更新する。

## 調査した表示経路

- `NagiAssistExternalAnnotator` の診断注釈が既存のmouse-hover本文を供給する。実compilerのread-only assistで `error: line 2:12: 式が必要です (found Sym(")"))`、` --> path:2`、` 2 | source` がmessageに入ることを確認した。parserのtoken Debugはここへ到達する。
- 型エラーは `expected i64, got str` の人向けDisplayだった。productionの型Debug、pluginによるmarkdown/backtick/HTML entity追加は確認できなかった。利用者が見た実GUIのentity表示原因までは確定していない。
- DocumentationProviderはなかった。Completionはcompilerのtype/signatureをplainなtypeTextとして表示し、Parameter Infoはcompiler parametersを表示するが戻り値を表示していなかった。必要なNagi型構文 `[]` / `?` は余計な記号として削除しない。

## 採用した境界

Sol 6.1 Highのread-only事前設計後、親が実装を承認。sourceの型・callable・所有者・説明文をtokenから推測しない。compiler assistが提供するdefinition、parameters、return_type、asynchronous、fields、reference位置だけを使う。additive fieldがない旧responseでも動作し、欠けた情報は表示しない。

Quick Documentationはhoverの実offsetを使う。caretと同じ位置であることを要求しない。定義名とreferenceは正確なUTF-16位置をcompiler graphで照合し、名前の一致だけでは表示しない。異なるdefinition/targetが同じ位置に衝突した場合はfail closed。virtual stdlib定義の表示は物理sourceへのnavigationに依存しない。

関数はcompiler signature・引数・戻り値を示す。classはchecked fieldsを示す。High class構築の全field名前付き指定は[言語ガイド](../language-guide.md)に従い、空parametersをゼロ引数constructorへ変換しない。resource/enumには既存signatureを表示し、未提供のconstructorや説明文を追加しない。

`fresh(file)` / `isCurrent` と既存workerを使用し、trust・generation・compiler identity・loaded source graph・disk/document invalidationを迂回しない。cold時はrequestNavigationを要求しnullを返す。EDT/read actionでprocessを起動したり完了を待ったりしない。初回warmup後は再hoverまたはQuick Documentationの再実行が必要で、自動再popupの新しいlifecycleは追加しない。

診断のmessageとannotation descriptionは不変。tooltipだけを固定HTMLで表示し、compiler入力を一度escapeする。既知のline headerと、structured file/lineに一致するorigin/gutterを読みやすく表示する。完全認識できる末尾token Debugだけ変換し、未知形式、Rust escape、notes/hintsを保持する。任意HTMLや既存entityをdecodeしない。表示整形は診断spanやnavigationの権限を増やさない。

Parameter Infoは既存return_typeがある時だけ戻り値を追記し、引数highlight位置を維持する。CompletionのtypeTextはそのまま保持する。

completion lookup専用のdocumentationは今回追加しない。lookupが閉じるまでsnapshotを保持するpresentation PSIの寿命管理を増やさず、source hover/Quick Documentationを対象にする。

## 必須検証と限界

rendererのHTML/entity/Unicode/未知形式保持に加え、登録Provider経由の実hover offset、定義名・reference・class・virtual stdlib・shadowing、cold/stale/untrusted、衝突、optional fieldのboundを検証する。診断の各range/file-level分岐で元descriptionとescaped tooltipを検証する。既存assertを緩めない。

SDK 251でのJVM tests、実compiler assist連携、ZIP生成、Plugin Verifierは別の検証段階として記録する。headless testsは実GUIのmouse表示・テーマ・IDE設定の受入試験を代替しない。未実施のIDE/版の成功を主張しない。

## ローカル検証記録

2026-10-22、IDEA IC 2025.1.1 / build 251.25410.109、Java API/class target 21。compilerは0.2.1作業のretained release CLI（`integrated-a6/nagic-release`）。compiler assistの意味論はmainと同じで、未統合PRのparser token保持差分だけを含む。compiler source・CIはこの差分で変更しない。

- focused 26件成功（19秒）。実compiler integration 10件成功（25秒）。後者にはHigh/Low関数・class・asyncとnamed importのoriginal/alias/use/module.member→documentation接続を含む。
- 全plugin 214件、failure/error/skip 0。その後、追加したSwing `JEditorPane` HTML renderer検査を含む表示test全6件も成功（10秒）。追加の新規1件を合わせて215件を検証したが、215件全体を1回で実行した結果とは区別する。literal `<tag>`、文字としての `&lt;`、引用符・apostrophe、日本語・絵文字・Rust escapeが表示documentに保持された。escape実装の推測変更は不要だった。
- 最初の `test buildPlugin` はtest成功後、`buildSearchableOptions` の `/home/agent/.local/share/JetBrains` read-onlyで失敗（全体67秒）。HOMEを維持し、専有tmpのXDG data/config/cacheを指定したpackaging再実行は21秒で成功。IDE背景接続警告はraw logに保持する。
- ZIPのdescriptor ID/version、High/Low Provider登録2件、表示class2件を確認。候補ZIP SHA-256: `495e1a07edd3405536b384d6edd4385da7fac08c503c9b92ec53af35af01dcdc`。版更新・公開は行っていない。
- Plugin Verifierの初回は解析開始前に `/home/agent/.pluginVerifier` read-onlyで失敗。現行CLIの `plugin.verifier.home.dir` 固有propertyを確認し、専有tmp指定でIC251解析へ進んだが、子Javaのremote依存解決でConnection refusedを観測。proxy設定の引継ぎ調査中で、Verifier gateは未完。TLS検証や検査は無効化しない。

raw logは作業環境の `local-only/plugin-documentation-{focused,integration,full,html-renderer,package,verifier,verifier-retry}.log` に保持する。SDK取得中のbootstrap試行は編集中sourceも含むため厳密なmain baselineや性能比較に数えない。他IDE/版のCI、独立最終audit、実GUI受入は未完gateとしてPRで追跡する。


## 入力中の色と標準hover経路の回帰（2026-10-22、未リリース）

利用者から候補ZIPでhover無表示と入力ごとの色消失が報告された。compiler設定の切替だけではhoverは解消しなかったとの報告を維持する。headlessのProvider直呼び成功を実GUI表示成功と扱わない。

色消失はSDK 251の実ExternalToolPassで再現した。解析待ちの空annotation結果も既存highlighterを置き換えるため、単にcollect/applyを省略する方法では維持できない。新しいpresentation storeは最後にpaintしたrole属性・token文字列/種別・RangeMarkerだけを所有する。世代照合はresponseへの参照ではなく、responseごとに割り当てる独立opaque identityを使う。型、reference、診断、compiler responseそのものを保存しない。soft editは接触範囲（境界への挿入を含む）と他documentのmarkerを破棄し、残ったmarkerにも現在の完全tokenとの一致を要求する。

storeは128 documents / 20,000 roles / token当たり256 UTF-16 code unitsで制限する。dependency/manifest、VFS、config/compiler、trust、file close、service disposeはhard invalidationとなり全markerをdisposeする。fresh失敗・新responseも旧presentationを破棄する。fresh/current判定、350ms診断・80ms補完debounce、worker取消、診断の元descriptionと位置のfresh条件は維持する。paint途中の取消・例外でも作成済markerを解放する。trust喪失時のpaintは既存invalidate境界を使い、再信頼後に破棄済presentationへ旧cacheを紐付けたままにしない。

modern hoverの試験は実compiler factsを使い、SDKのIdeDocumentationTargetProvider service→DocumentationTarget.computeDocumentationをactual hover offsetで呼び出す。workerのindicator→runBlockingCancellable→blockingContext→read actionというSDK所定のcontextを使い、caretは別位置に置く。testのtimeout/例外時にはindicatorとFutureを取消し、Callableのfinally latchで実終了を有界に確認する。最初の試験harnessはJob context不足で失敗したが、これはprovider不具合の証拠としない。warmの関数説明は標準経路で成功した。GUIのmouse設定・実popup受入は引き続き未確認。

coldのnull応答後に同じhoverを更新しない制限は未解決。標準async DocumentationTargetで既存workerの完了を待つ案は別の承認・実装段階に分ける。builtin printは現compilerのdefinition/reference graphに署名を持たず、実helloのprint位置へ送ったcursor requestもcompletion署名を返さなかった。call expression結果unitを関数署名として転用しない。localsにはexact location/typeがあるが、この色修正では新たなhover情報を追加しない。

実測logは local-only/pr148-stability/ に保持する。real annotation passの保持assertは変更前RED→修正後GREEN。初回fullは224中223成功、1件は新trust復帰caseで失敗し、上述の失効処理を修正後targeted 6件成功。20,000同位置markerを生成する病的な上限fixtureはCPU負荷が大きいため所有test processを停止し、上限直前のcountをtest-only reflectionで置く境界試験へ置き換えた（製品の上限とassertは維持）。正式全suiteの最終結果は後続記録に確定する。cold async、run profile、compiler変更はこの区切りに含めない。

正式最終検査: IC 2025.1.1 / SDK251、Java21、`NAGI_TEST_COMPILER=/workspace/Nagi/target/release/nagic` で全plugin 225件、failure/error/skip 0（71秒）。保存fixtureの既存NagiSaveBarrier追加も含む。`git diff --check` 成功。最終raw log: `local-only/pr148-stability/full-final.log`。新sourceのpackaging/Verifier・exact HEADの4IDE CI・別担当の独立audit・実GUIはこの時点では未完。

opaque identityへの最終3行修正後にも全225件を再実行し、failure/error/skip 0（71秒、`full-opaque-generation.log`）。旧responseへの不要な強参照を保持しない。Verifierはcold asyncの後続差分後、最終sourceでまとめて行う。cold async開始前にこの色修正を凍結し、別Highの独立auditへ渡す。


## cold要求の非同期完了とlocals（2026-10-22、未リリース）

利用者はActions ZIPを試し、本体0.2.0を正しく指定した後も「全部」のhover説明が出ないと報告した。printだけの問題と扱わない。初期版のProvider直呼び/warm成功では、coldでnullを返した後に同じ要求を更新しないDX不足を検証できていなかった。SDK251のEditorMouseHoverPopupManagerはmouse設定を満たすactual offsetのPSIを取り、標準DocumentationTarget consumerへ進む。今回の実テストはplugin.xml登録EPがNagi/NagiLowそれぞれで新Targetを選んだことと、SDK consumerが同じcold要求のasync resultを最後まで解決することを検証する。実GUI表示の受入をこのheadless検査へ置き換えない。

標準DocumentationTargetProviderはNagi sourceだけに応答する。TargetはSmartPsiPointer・actual token位置/長さ・Document stamp・service epochを保持し、型/Definition/旧CachedResponseを保持しない。warmは現在の既存renderer、coldはSDK asyncDocumentation Supplierを返す。Supplierはread action外でSDK awaitWithCheckCanceledを使い、前後にも取消を確認する。最後の短いreadでpointer/範囲/stamp/epoch/trust/fresh/isCurrentを再検査してから表示する。EDT待機、read lockを保った待機、poll/sleep、新たなretry loop、強制popup、グローバルIDE設定変更は追加しない。旧DocumentationProviderの互換経路も維持する。

待機ticketは最大8件、FutureはBoolean readinessだけを渡す。同じfile/Document stamp/epoch groupの解析を共有し、cursor query置換では引き継ぐ。登録/publishはread→service lock順でlost wakeを防ぎ、terminal通知はlock内でdetachした所有者がlock外で行う。private deadline futureは独立JDK executorでcheck timeout＋350ms（1,350〜300,350ms）を待ち、compiler workerが停止中でもregistryを解放してから公開signalをfalse完了する。publish/失効/cancelはprivate deadlineも取り消し、sourceやticketを期限まで不要保持しない。AppLevel trust失効通知は対象projectだけをhard invalidateし、待機・cache・role store・daemon表示を更新する。

新dependencyを初めて報告した結果は既存どおりdiscardし、precaptureをやり直す既存予約を使う。この予約が受理された1分岐だけ、origin attemptと同file/Document stamp/epochへ束縛するscalar tokenでticket責任を移譲する。新しいretryを増やさず、最初の独立deadlineを延長しない。再予約の拒否、callback失効、generic STALE、編集・VFS・設定・trust・failure・disposeは解除する。owner identityを使い、遅れて終わった旧attempt/finally/callbackが新ticketを消さない。tokenにも旧source snapshotやcompiler factsを保持しない。

localsは既存protocolのname/type/exact locationだけを使い、現在の完全tokenと一致する位置へ型を表示する。definitionとの衝突、異なるlocal facts、異なるreference targets、曖昧な定義はfail closed。名前だけでbuiltin/alias/型を判定せず、local型から関数の引数・戻り値を合成しない。builtin printは署名がないため未表示の制限を明記する。

検証中の失敗も保持する。CRLF文書のIDE正規化に対するfixture offsetをactual PSIへ訂正し、Lowの不正な// commentを正式な#へ直した。CRLF生成のassertはIDEがdiskを正規化する前のwrite直後に置く。取消testのApplication.executeOnPooledThreadはSDK自体がPCEを正常nullへ変換するため、raw AppExecutor submitとactual SDK Deferred取消へ是正し、例外/他ticket生存/compiler非取消のassertを維持した（製品取消不具合とは断定しない）。timeout完了とregistry解放の競合もprivate deadline＋detach所有で修正した。

公開Nagi 0.2.0実配布binaryでもHigh/Lowのcold関数/class/localsとasync/戻り値、renderer/index負検査が成功。初回35 focused中の残1件は公開版がbare module `from helper import ...`を拒否する文法差で、両CLIが受理する `from "helper.nagi" import ...` へfixtureを訂正し、公開版の同cold SDK consumerも成功した。bare module文法が公開0.2.0対応だとは主張しない。14件のticket検査では上限8/9、共有worker、別ticket取消、独立timeout、解析失敗/拒否、同group query置換、別file置換、edit/dependency/config/trust/close/dispose、old pointer、safe recaptureと遅延finally/rejected enqueueを確認する。raw logは `local-only/pr148-stability/async-*.log` とquoted-import JSONに保持する。正式全suite/ZIP/Verifier/独立audit/最新HEAD CIとGUI受入は別gateで確定する。


cold最終sourceの全plugin243件、failure/error/skip 0（84秒、`async-full.log`）。その後、新AppLevel listenerが明示dispose済みserviceのCLOSEDを上書きするcaseを追加assertでRED再現し、disposed/projectDisposed guardを追加してtargeted GREEN（16秒）。この最小修正後の全体・ZIP・Verifierを続けて検証する。旧失敗log/JSONを削除せず、未確認GUIと最新HEAD CIを残す。


最終guardを含む正式全plugin243件、failure/error/skip 0（77秒、`async-full-final.log`）。`git diff --check` 成功。source/testの変更はここで凍結し、同sourceのZIPとVerifierを検証する。新規run profile・compiler・CI製品変更は含めない。


同sourceのZIP生成は21秒で成功、SHA-256 `30293d30acac49be8ee12a7dfe655a4a075698cde07d24f96f841f88c58e183e`。実ZIPから新async EP登録1件・新TargetProvider・旧Providerを確認した。IC 2025.1.1 / 251.25410.109のPlugin Verifierは11秒でCompatible（deprecated API 3、experimental API 12）。標準Documentation/TargetPresentationとtrust listenerを含むexperimental APIの版変更リスクは残し、4IDEの最新HEAD CIで検査する。HOME/TLSと検査を維持し、専有Verifier home、既存proxyのhost/port、SDK/cacheを再利用した。raw log `async-package.log` / `async-verifier.log`。source/test/docsを凍結し、別Highの独立audit・最新HEAD CI・利用者の実GUI受入へ渡す。今回のheadless成功を利用者の実mouse表示確認と報告しない。
