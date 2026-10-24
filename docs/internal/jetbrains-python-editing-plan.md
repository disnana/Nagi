# PyCharmのPython編集補助とNagiへの適用

2026-10-22調査。Nagi側の基準はmain `5d3e58e`。名前付きimportのPR #141は別の未統合作業として扱う。

## 2026-10-24: 連続Enterの独自動作を撤回

利用者の指示により、自動字下げ後の連続Enterでブロックを退出する動作を撤去する。
これはNagiが独自に追加した操作であり、PyCharm Pythonと同じ動作として案内しない。
空行ではEnterを何回押しても現在の字下げを維持し、手動dedent・通常のブロック/括弧補助・
Low・Smart Indent設定・undo/redoを保つ。editorのEnter履歴と変更sequenceへの依存も除去する。

統合版PyCharm 2026.2.3（PY 262.10968.92）の実editor actionで、変更前は新しい期待4件が失敗し、
撤去後は入力関連6 suite・81件が成功した。既存の退出期待を字下げ維持へ変更した根拠は
この明示した操作契約の変更であり、検査失敗を隠すassert緩和ではない。
以降の比較・実装境界は0.1.4実装時の履歴で、連続Enter退出に関する記述はこの撤回が優先する。

## 根拠と範囲

JetBrains/intellij-communityのPython実装・登録を調査した。参照commitは
`56f1f5c09bece0c124a3684a64c189b4532a1054`。
これはsource比較であり、Pythonファイルを実PyCharmで操作した同等性試験ではない。
製品・版・設定により動作が異なるため「PyCharmと完全同等」とは主張しない。

主な参照:

- [Enterと文字列・docstring処理][python-enter]
- [手動dedentを尊重するEnter][python-indent]
- [Pythonの構文に基づく整形・改行の字下げ][python-block]
- [コロン入力時の分岐補正][python-keyword]と[owner探索][python-unindent]
- [構文・編集拡張の登録][python-syntax]
- [補完・参照・inspection等の登録][python-psi]
- [Smart Keys設定][python-settings]、[折りたたみ][python-folding]

`PyEnterAtIndentHandler`は字下げ内でのEnterに`DefaultSkipIndent`を返し、利用者の手動dedentを尊重する。
`PyBlock.dedentAfterLastStatement`はreturn/raise/pass/ellipsisを扱う。
登録されたEnter処理には「空行で2回目のEnterなら必ず1段下げる」という規則を確認できなかった。
連続Enterによる退出は今回の利用者要望によるNagi独自補助として実装し、Pythonからの移植と混同しない。

## 対応表

「既存」はmainの実装があることを表し、Python全機能との同等性を表さない。

| 操作・機能 | Nagiの既存状態 | 適用・判断 |
|---|---|---|
| `:`後のEnter | Highのブロックを1段下げる | 維持。幅は有効なCodeStyle/EditorConfigに従う |
| return後のEnter | Highで1段戻す。複数行returnも対応 | 維持。Python専用のraise/pass/ellipsisは追加しない |
| 手動dedent後のEnter | 空行から過去のblockを再推定し、手動位置を戻す場合があった | **今回改善**。Highの明示的な空行の字下げを維持する |
| 連続Enterでblock退出 | 字下げを維持していた | **今回追加**。自動字下げ後、document・caretが変わらない連続Enterだけ1段戻す |
| 括弧内のEnter | 継続字下げ、閉じた複数行式から元の段へ戻る | 維持。連続Enterでも括弧内から退出させない |
| `else:`補正 | NagiのJetBrainsプラグインでは未対応。Nagi VS Codeには実装あり | **今回追加**。有効な近傍ifに合わせる。既にelseを持つifを再利用しない |
| `case ...:`補正 | case後のEnterのみ対応 | **今回追加**。matchの1段内側へ合わせる。複数行headerの相対位置を保持 |
| 閉じ括弧の入力 | 対の挿入・skipは対応。独立した閉じ行の整列は不足 | **今回追加**。Highの`)`/`]`、Lowの`)`/`]`/`}`を開始行の段へ合わせる |
| Tab/Shift-Tab | IDE標準actionとNagiのspace-only幅設定 | 維持。全ファイルの再整形をTabへ結び付けない |
| smart Backspace | AUTOINDENTをIDEのINDENTへ接続。OFFを尊重 | 維持。手動Backspace後のEnterを新しい自動退出として扱わない |
| HOME/END、行移動・複製 | IDE標準action | 言語固有overrideを増やさない。具体的不具合があれば実actionで再現する |
| 括弧・引用符の対入力 | 挿入・skip・削除・選択範囲の囲み、設定OFFに対応 | 維持。Pythonのtriple quote/f-stringはNagiにない |
| 選択範囲拡大 | IDE標準とtoken PSI | 構文単位の拡大は将来。Python AST選択handlerの流用は不可 |
| コメント | `#`行コメント | 維持。docstringをコメントと同一視しない |
| 折りたたみ | Highの字下げblock、Lowの複数行brace | 維持。Python専用構文のfoldは対象外 |
| 明示live template | `def`/`async def`/Low `fn` | 対応済み。statement用if/match/scope等は正しい展開文脈を用意して別差分で追加する |
| 名前・型に基づく補完 | compilerのsnapshotに基づく候補、先頭def/fnの字句候補 | 既存の正確性・trust・取消境界を維持。Python名解決を再実装しない |
| 引数情報 | compilerのsignatureに基づくParameter Info | 既存。型・ownershipの修正候補はcompilerの根拠を追加してから表示する |
| 定義移動・semantic色分け | 正確なsource targetがある場合に対応 | 既存。推測で別moduleへ飛ばさない |
| 名前付きimport補完 | mainでは未統合。#141で別途対応中 | 今回の編集補助PRに混ぜない。Python package探索とは別契約 |
| 自動import・import最適化 | 未対応 | 公開/re-export境界・副作用・alias・順序の契約と、compilerが返す編集案が必要 |
| Find Usages / Rename | 正確な全参照索引はなく、referenceはrenameを明示拒否 | compiler側の安定symbol IDと全参照・未保存buffer・alias・shadow対応が先。文字列置換では実装しない |
| Extract/Inline/Safe Delete | 未対応 | ownership・評価順・async取消・可視性を保存するcompiler解析と編集検証が必要 |
| Surround With / postfix補完 | 専用descriptorは未対応 | Nagiの対応する構文だけを対象とし、選択範囲・評価順・型を確認する設計が必要 |
| Paste/Reformat | space-only設定、基本paste・reformat fixture。全構文formatterなし | Highでは字下げが意味を持つ。Python formatterの一括移植を避け、Nagi formatter設計を別途行う |
| Inspection / quick fix | compilerが返す最初の診断と位置を表示 | 複数診断・修正編集をprotocolへ追加する設計が必要。ownership/PolicyをIDEが推測しない |
| ドキュメント・structure view | Python同等の全機能はない | compilerの宣言・ドキュメント・位置を返すprotocolを優先。snapshot検証と取消を再利用 |
| テスト生成・実行・debug | Run/Check/Stopと標準Run Configurationあり。Python同等のtest/debugなし | Nagi専用test protocol・位置対応を先に定義。pytest/unittest/debugpyをそのまま依存にしない |
| Interpreter/venv/pip支援 | compiler pathとNagi project設定 | Python固有環境を移植しない。Nagiのtoolchain・依存・実行設定を改善する |

## 今回の実装境界

- Python plugin APIは使わず、既存のIntelliJ Platform依存を維持する。
- Enter履歴はeditorごとにmodification stamp・変更sequence・caret offsetだけを記録する。
  手動入力、paste、Backspace、undoなどのdocument変更後は連続Enterとして扱わない。
  変更sequenceを実装せず恒常0を返すdocumentでは追加退出を無効化する。実sequenceが0から増加する通常文書は初回Enterから記録できる。
  multi-caret・選択範囲・Smart Indent OFFでも追加の退出を行わない。
- 連続Enter退出はHighのみ。Lowはbrace文脈を維持する。
- 分岐と閉じ括弧はtokenから確認できるleading whitespaceだけを変更する。
  文字列・コメント・無関係なstatementや相対的な継続位置を壊さない。
- typed補正は完全なprefixを最大8192 UTF-16文字・96行・4096 tokenで確認する。
  途中から切り取った未知の括弧・block文脈を推測しない。
  Enterの既存の上限付きhighlighter走査も維持する。
- 上限外、mismatch、不明文脈では通常入力を残す。compiler起動、独自のfile I/O、全体再parseをkey入力経路へ追加しない。

## 次の段階

1. 今回のEnter・分岐・閉じ括弧補正を実IDE actionで検証し、小さな差分として提供する。
2. 長いファイルの入力補助には、既存lexerを利用する変更追随の構文文脈索引を設計する。
   snapshot/version・更新範囲・上限・不確定状態を明示し、全prefix再走査や無制限なkeyup処理を避ける。
3. statement template、構文単位の選択・surroundを、Nagiの対応構文と明示操作から追加する。
4. formatterはHigh/Lowの値・評価順・コメント・文字列・字下げの意味を保存する独立設計とする。
5. Rename・Find Usages・自動import・quick fixはcompilerの根拠を返すprotocolから実装する。

## 検証

実エディターfixtureではEnter連打、手動dedent、括弧継続、Low、undo/redo、設定幅、設定OFF、
分岐のowner・消費済みelse・複数行case、閉じ括弧の入力/skip、文字列・コメント・Unicode・
選択範囲・multi-caret・不明/上限文脈を検査する。Java compile、実fixture実行、Plugin Verifier、
GUI上の見た目・IMEを別々に扱い、未実行の段階を成功と数えない。

ローカルの実IDE action・compiler連携を含む全188件と候補ZIP生成/descriptor検証は成功した。
入力時間の条件・raw・RED/GREEN・未確認範囲は[観測記録](../../editors/jetbrains-nagi/benchmarks/python-editing-2026-10-22/README.md)を参照。

[python-enter]: https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/python/python-syntax/src/com/jetbrains/python/editor/PythonEnterHandler.java
[python-indent]: https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/python/python-syntax/src/com/jetbrains/python/editor/PyEnterAtIndentHandler.java
[python-block]: https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/python/python-syntax-core/src/com/jetbrains/python/formatter/PyBlock.java
[python-keyword]: https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/python/python-syntax/src/com/jetbrains/python/codeInsight/PyKeywordTypedHandler.java
[python-unindent]: https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/python/python-syntax-core/src/com/jetbrains/python/codeInsight/PyUnindentUtil.java
[python-syntax]: https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/python/python-syntax/resources/intellij.python.syntax.xml
[python-psi]: https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/python/python-psi-impl/resources/intellij.python.psi.impl.xml
[python-settings]: https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/python/src/com/jetbrains/python/codeInsight/PySmartKeysOptions.kt
[python-folding]: https://github.com/JetBrains/intellij-community/blob/56f1f5c09bece0c124a3684a64c189b4532a1054/python/python-syntax/src/com/jetbrains/python/PythonFoldingBuilder.kt
