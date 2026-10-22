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
