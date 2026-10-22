# JetBrains hover / Quick Documentation

2026-10-22。base main `765f0b0`。compiler/CIを変更せず、plugin側でcompiler事実の表示を追加する未リリースの作業。

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
