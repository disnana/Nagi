# IDE補完・型色の語彙照合（2026-10-26、開発中）

基点はmain `d7dec56791be22d733017dfbab3427091d9265b6`。公開compiler 0.2.1／JetBrains plugin 0.1.6の機能追加済みとは扱わない。open PR／issueの重複を確認し、当時openだった#169は対象外とした。

## 語彙と実provider

| 対象 | 基点の状態 | この差分 |
| --- | --- | --- |
| JetBrainsのキーワード | 登録済contributorは最上位`def`／`fn`だけ | `try`を含む共通語彙、High `class`／Low `record`・`let`、case直後の`Ok`／`Err`／`Some`／`None`を字句候補にする。`def`／`fn`は`async`／`extern`の後にも対応 |
| JetBrainsの組み込み関数 | compiler assistのinventoryが`print`／`read_line`の2個 | checkerのscope・taint判定を経る全43 intrinsic候補。関数値／definition targetを追加しない。更新したcompilerが必要 |
| 組み込み型 | 両IDEに欠落／廃止名がある | 数値10型、`bool`・`str`・`bytes`・`unit`・`Error`・`UUID`・`timestamp`・`List`・`Result`・`view`・`shared`・`Option`・`Map`・`fn`の24表記を照合 |
| class／enum名 | JetBrainsはcompiler応答まで宣言名が通常identifier。VS Codeの宣言regexは空白1個限定、Lowの`record`名は欠落 | 宣言名を字句で型色にする。JetBrainsの参照は既存のexact compiler factsを維持。VS Codeの宣言は複数空白／tabに対応し、High／Low文法の共有部分から言語別宣言を分離 |
| 廃止API | VS Codeに`html`／`serve`／`db_*`、`Html`／`Db`が残る | 候補・文法から除く。compilerのmigration拒否は維持。型`owned`は公開説明で未完成、`Range`／`Future`は内部表現のため新しい推奨候補にしない |

全43 intrinsicは`print`, `write`, `read_line`, `include_text`, `view`, `copy`, `share`, `clone_shared`, `len`, `range`, `sleep`, `json_decode`, `json_encode`, `ok`, `some`, `error`, `not_found`, `internal_error`, `fail`, `error_kind`, `error_message`, `assert_true`, `parse_i64`, `parse_f64`, `make_ints`, `actor_demo`, `actor_pair_demo`, `queue_demo`, `task_demo`, `cpu_sum`, `i64`, `i32`, `uuid_parse`, `uuid_format`, `append`, `env`, `slice`, `bench_i64`, `bench_f64`, `bench_scalar`, `clock_ns`, `size_of`, `supervisor_demo`。checker arity表、assist実応答、VS Code既存arity照合test、実TextMate tokenizerを別々に確認する。

JetBrainsの字句候補はcompilerの起動／trustを要求しない。member／import先／宣言名へ一般キーワードを混ぜず、string／commentの記号を構造と扱わない。prefix／周辺走査は8192 UTF-16 code units以内、宣言判定は既存のbyte・token・depth・時間上限を使う。IDEの通常lookupを使用し、自動確定・live template展開を強制しない。型候補は型注釈位置に限定し、候補が文全体の型検査成功を意味するものではない。

## 括弧の深さ別色：調査と残件

- [VS Code公式native実装解説](https://code.visualstudio.com/blogs/2021/09/29/bracket-pair-colorization)と[言語構成guide](https://code.visualstudio.com/api/language-extensions/language-configuration-guide)を確認。既存`language-configuration.json`の`brackets`はHighの`()`・`[]`、Lowの`{}`・`()`・`[]`を登録済み。[最小対応VS Code 1.85の公式languageConfiguration型](https://github.com/microsoft/vscode/blob/1.85.0/src/vs/editor/common/languages/languageConfiguration.ts#L65-L69)も、`colorizedBracketPairs`を「nesting levelで色付けするpair」、未指定時は`brackets`を使用すると明示している。IDE native engineへ委譲し、`editor.bracketPairColorization.enabled`、themeの`editorBracketHighlight.*`、既存のaccessibility設定を尊重する。global設定変更、独自decoration、第三者pluginの変更は行わない。
- [JetBrains公式色設定help](https://www.jetbrains.com/help/idea/configuring-colors-and-fonts.html)のsemantic highlightingはparameter／local variableへ名前別の色を割り当てる機能。[公式SDKのhighlighting guide](https://plugins.jetbrains.com/docs/intellij/syntax-highlighting-and-error-highlighting.html)の`RainbowVisitor`／`RainbowColorSettingsPage`も同じ用途。`RainbowVisitor`は`com.intellij.highlightVisitor`へ登録する。括弧nesting depth用APIとして流用しない。
- 対応最小IDEA 2025.1.1（IC build `251.25410.109`）の実SDKで`PairedBraceMatcher`、`BraceMatchingUtil`、`RainbowVisitor`／`RainbowHighlighter`を確認した。既存の`NagiBraceMatcher`は`com.intellij.lang.braceMatcher` extension pointで公式[`PairedBraceMatcher`](https://github.com/JetBrains/intellij-community/blob/idea/251.25410.129/platform/analysis-api/src/com/intellij/lang/PairedBraceMatcher.java)へtoken pairを登録する。`getPairs`・`isPairedBracesAllowedBeforeType`・`getCodeConstructStart`はpair定義、入力補助条件、構文開始位置を提供し、depth palette／toggleは提供しない。`lang.syntaxHighlighterFactory`のlexer／highlighterは括弧tokenを`PARENTHESES`・`BRACKETS`・`BRACES`のtheme keyへ対応させる。現行lexerはphysical-lineの字句状態で、ファイル出現順やnestingに色番号を割り当てない。これは対応括弧のmatchingで、nesting depthごとの連続色付けではない。公式supported depth色APIの確証は得られていない。
- public `Annotator`／highlighter APIで独自range色を描く機能は作れるが、native depth palette／toggleとの連携や第三者rendererとの共存を得られる証拠はない。独自の高頻度range管理・描画を追加しない理由は、この新しい所有・性能・設定契約が未確定なため。
- 現実的な選択肢は、IDE側の機能が利用可能になった版での再評価、将来の公式supported depth APIへの統合、利用者が既に選んだ対応pluginでの動作確認。第三者pluginとNagiの互換は未検証で、インストール／global設定変更は行わない。
- JetBrainsのdepth色はこのPR外の残件。将来のnative機能／公式API確認、または別機能の設計判断が必要。独自rendererを導入する場合にはtheme/toggle、第三者rendererとの競合、同pairの同色、comment/string、未完成／不整合括弧、incremental edits、取消・dispose、巨大／深いfileの負荷とaccessibilityの設計・実GUI受入を先に確定する。

VS Codeにはこの基点でsemantic-token providerがない。TextMateによる宣言色と組み込み語彙が対象で、任意のユーザー型参照のsemantic色付けは未実装。新providerのfreshness／cancellation／theme契約は別スコープの判断とする。

## 検証

- compiler assist: `cargo test --offline --locked -p nagic --test editor_assistance`、42 passed。独立手書きinventoryとHigh／Low実応答の全43候補、全intrinsicのlocal shadow、既存global／import／member／move／taint／UTF-16回帰を含む。
- VS Code: `npm test`、198 passed。既存実compiler testsを含む。別途公式`vscode-textmate@9.3.2`／`vscode-oniguruma@2.0.1`で両grammarを実token化し、全43 builtin・24 type・High／Low宣言・複数空白／tab・廃止API・comment/string／未完成行の216 assertionsが成功。
- JetBrains: IDEA 2025.1.1／JDK21、独立reviewの増分Lexer修正後、最終sourceの全323 testsが成功（failure／error／skipなし）。`buildPlugin`と同ZIPの公式Plugin Verifierを再実行して成功。登録済lookup経由でHigh／Lowの全43intrinsicとそのsignature、全keywords／types／`try`のRHS位置、member/import/comment/string／bounded scan、既存自動lookup toggle、型のexact位置とmalformed-edit回復を検査。
- 同ZIPの公式Plugin Verifier: IC `251.25410.109`でCompatible。既存deprecated API 3 usages／experimental API 13 usagesがあり、警告なしではない。最初のVerifier取得失敗は子JVMのproxy未継承、system CAとruntime proxyを継承して解消した。
- Rust全suiteの最初のsandbox実行は実socket bindのEPERMで停止。同じsourceをsocket利用可能な環境で再実行し、`cargo test --offline --locked`全125 result blocks、2059 passed／0 failed／4既存ignoredで成功した。all-target Clippyとfmtも成功。これは最初のsandbox制約を言語契約の失敗として扱うものではない。

他IDE stable／EAPのexact-head CI、Windows／macOSと実GUIでの配色・accessibility／native bracket表示は未確認。独立reviewとCIの完了前はDraftを維持する。版更新、Marketplace公開、tag／release、mergeは行わない。


## 独立review追修正：宣言名の増分色付け

最初の差分はscannerで直前keywordを見て宣言名をTYPEにした一方、`NagiLexer.getState()`が常に0だった。実editor documentで`class Point`／`record Point`のkeywordを同じ長さの通常identifierへ変更すると、増分highlighterがSPACEで旧token/stateへ合流し、Pointの旧TYPEが残った。逆方向とHigh／Lowの`enum`にも同じ問題があった。独立reviewのP2として受け入れ、追加した実document回帰5件のREDを確認した。

[公式Lexer guide](https://plugins.jetbrains.com/docs/intellij/implementing-lexer.html)と[SDK251のLexer.java](https://github.com/JetBrains/intellij-community/blob/idea/251.25410.129/platform/core-api/src/com/intellij/lexer/Lexer.java)は、token開始位置で返したstateを同じ位置の`start(initialState)`へ復元する契約を示している。

最初の追修正ではkeyword／空白／dotのstateを保持したが、scannerのlookbehind分類とstate遷移が別々に残った。再reviewで`;class Point`↔`1class Point`、Lowのrecordの同一長prefix編集にも旧色が残るP2が見つかった。追加した実document prefix回帰とallowed／blocked stateの識別回帰が両方REDになったため、数字だけの追加分岐ではなく分類経路を改めた。

最終実装はscannerの共通stateful分類と遷移へ集約する。Lexerは現在tokenと保存stateだけで分類し、過去textのlookbehindを行わない。CODE／宣言名待ち／隣接prefixの3状態をtoken開始時点で保持する。ASCII identifier文字・digit・underscore・dotの共通predicateで宣言keyword直前の阻害条件を表す。完結word tokenは隣接identifier文字をすべて消費するため、通常のCODE再開位置を維持できる。numeric／punctuation等のtoken境界に残るprefixをstateで保存する。宣言名待ちは水平空白／tabを越え、物理改行、comment／string、ほかのtokenと8192 code unitsの空白上限で解除する。既存stateless scanner利用者はboundedにcontextを復元して同じ分類関数を通り、同じkeyword／prefix predicate／上限を使う。全file走査や新rendererは追加しない。

[SDK251のRestartableLexer](https://github.com/JetBrains/intellij-community/blob/idea/251.25410.129/platform/core-api/src/com/intellij/lexer/RestartableLexer.java)も確認したが、一部methodがExperimentalのため新たには採用しない。通常word後のCODE再開位置を保ち、長いidentifier／dot chainでも不要に先頭まで戻らない。

実documentのincremental token列を、各編集後のfresh `LexerEditorHighlighter`全token列へ直接照合する。class／record／enumの同一長双方向編集、keywordの追加／削除、複数空白／tab、未完成keyword／単語結合、comment／string往復、semicolon↔digit／identifier／underscore／dot／隣接keyword、空白上限とSPACE／型名開始位置からのrestartを検査する。全token境界から保存stateで再開した列とfull lexを比較し、既存stateless scannerの分類とも照合する。LF／CRLFの実file bytesを確認してIDEへ読み込み、正規化されたLF Documentへ編集する。raw CRLF CharSequenceも直接restart検査する。IDE DocumentはCRLFの直接挿入を受け付けないため、入力経路をこの契約へ合わせた。

最終focused 57件（増分／restart 8件を含む）が成功し、その同一製品sourceで全323 tests／buildPlugin／同ZIPの公式Plugin Verifierが成功した。追修正はJetBrains Lexer／scannerと回帰testだけで、Rust／VS Code sourceは前回検証snapshotのSHA256と一致し、2059／198／216の証拠を再利用する。
