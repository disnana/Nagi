# JetBrains: 入力中のsemantic色を解析パスの寿命から分離する

基点はplugin 0.1.5公開commit `2e983e2`。2026-10-24の利用者報告は、正常なPointの入力・改行で色が一瞬消えて戻ることと、連続Enterでの独自dedent撤去である。ホバーは利用者が正常動作を確認したため、documentation providerの製品処理は変更対象にしない。この差分は未リリースで、公開済み0.1.5のtag/ZIPを変更しない。

## なぜ既存の色保持だけでは足りないか

0.1.5は公開Nagi 0.2.1のcursor回復で失われるsymbol indexを、同じsnapshotのqueryなし要求で回復する。これはcompiler情報の欠落への対策であり、IDEが画面のhighlighterを置換する経路とは別である。

PyCharm 2026.2.3（統合版PY、build `262.10968.92`）と、その[公式source commit](https://github.com/JetBrains/intellij-community/tree/404a109fe630a2b9013f10b71081f87c9dbb32e2)を確認した。ExternalToolPassは同一fileのキュー更新をreject/coalesceした際も、古い注釈を除去するため空のholderで完了する場合がある。ExternalAnnotatorへ色を返す修正だけでは、そのapply自体が呼ばれない経路を防げない。

さらに、collect時とapply時のWAITING/RUNNING/READY差や、新responseの公開時に保持markerを先に消す処理にも、未変更箇所の色が空になる窓があった。

## 採用する境界

- semanticな**表示色だけ**を、plugin所有のhighlighterとして管理する。診断と状態警告はExternalAnnotatorへ残す。
- 保持するのはrange・token文字列/種別・色のkeyだけとする。型・診断・ホバー説明・参照先は保持色から作らず、従来のcurrent snapshotだけを使う。
- 同じfileの新response公開から次のcurrent paintまで、未変更tokenの色を維持する。完全なcolor index（`frontendAccepted && hasSymbolIndex`）は空結果も含めて以前の色を置換する。`semantic_status=editor-partial`はcursor情報の状態であり、index全体の完全性には使わない。
- 閉じていない呼び出し/文字列や型エラーなど、通常の入力途中でindexが未取得・不完全な場合は、字句確認できる色だけを保つ。既存SemanticIndexが許可した現在のprovided colorsを優先し、重ならない未提供spanへ保持色を上限内で補う。型・説明・診断・参照のeligibilityは変更しない。
- tokenへの編集、comment/string化、依存・manifest・設定・compiler・trustの変更、通信/プロトコル/実行失敗、file closeとdisposeをそれぞれ検査する。未commit文書の妥当性を古いPSIだけで判断しない。
- token境界へ挿入するだけでは破棄せず、非greedyなrangeの移動後に現文書の完全tokenを照合する。空白/Enterで同じtokenが残る場合は保持し、識別子の結合・分割・comment/string化や証明できない範囲は破棄する。従来の境界一律破棄からの変更は、未変更の名前の色を安定させる利用者要求に基づく内部表示契約の更新である。
- IDEのTextAttributesKeyを使い、配色設定を尊重する。色の層が診断の強調表示を隠さない。既存の保持量の上限、取消と解析時間の制限を維持する。
- 色層とmarker storeの置換がそろうまで旧状態を変更しない。途中の取消/準備例外では新しく準備した要素だけを破棄し、hard clear後に旧色を復活させない。presentation storeに保持するmarker上限20,000は維持し、置換準備中だけはstore用の旧集合と新集合の最大40,000を許す。表示用highlighterの一時style集合もprotocolの予算内に限定する。これはIDE全体のRangeMarker数やRSS上限を表す値ではない。
- 新highlighterはSDKの`afterAdded`通知より前のcallbackで一時所有集合へ登録する。listenerが直接取消/例外を投げ、追加APIが戻らなかった場合も準備済みrangeを解放し、旧store/旧表示を維持する。

Nagiのparse/type/ownership/Policy検査、compiler protocol、Cargo、実行profileには変更を加えない。色は部分的な編集支援であり、保持された色からプログラムの受理や型の鮮度を推定しない。

## 検証の段階

旧実装で、新response公開から次のpaint前にEnterするケースの色消失を確認した。正規daemon session内で実ExternalToolPassを生成・collectし、同一fileの実キューでcoalescing/rejectさせたケースも、未変更箇所の実MarkupModelの色が消えるassertで失敗した。fixtureのbackground/context不足による先行失敗とは分けて記録する。

修正後、この実経路と通常の収集/適用間の状態遷移、未commit編集、明示的に完全な空index、hard invalidation、UTF-16、複数editor、scheme keyと診断layer、空白/Enterの境界保持と単語結合/comment/string化の拒否を確認した。さらに、置換準備中の取消・factory例外・hard clearの5失敗を追加して修正した。型・docs・診断・navigationの旧情報拒否も維持する。

公開Nagi 0.2.1の実Pointでは、未完成call/quote/type errorの新responseによって、別位置の未変更Point色が消える3件もRED再現した。入力の不完全さと完全な空indexを区別する変更後は、実MarkupModelの色消失がなくなり、partial provided優先・未提供名保持・上限・取消を含む対象30件が成功した。空のprogramがacceptedかつ明示空indexを返すことも実応答で確認した。最終の全plugin 306件も、current sourceと同じcompilerを使ってfailure/error/skipなしで成功した。

独立監査では、SDKの`afterAdded` listenerが直接PCEまたはRuntimeExceptionを投げると、追加APIの戻り値を後から所有集合へ入れる方式では登録済みrangeが残る問題を指摘された。両例外を実listenerから直接投げる2件でREDを確認し、通知前のcallback内で所有権を登録する方式へ変更した。各ケース専用のlistener寿命で、seed済み/未seedの両方について、元の例外の伝播・旧store/旧色の維持・新rangeの解放・hard clear後の残留なしを確認した。使用するkey付きAPIと通知順序はSDK251/262/263のsourceで確認し、実行検証はPY262で行った。

全plugin tests、共通ZIP、Plugin Verifierと最新HEAD CIは別の完了条件とする。テスト件数だけで、表示の連続性を確認したとは扱わない。Run fixtureの最初の全体検査では、SaveBarrier退出をstartupの終端と誤って同一視した待機でPENDINGを読み失敗した。製品Runの処理とREJECTED/failure型/未起動のassertを維持し、既存5秒待機でterminalを明示確認する修正後、関連22件が成功した。原失敗は中間証拠として残した。

公開Nagi 0.2.1と日本語・空白を含む実fileで、mouse documentation入口からDocumentationBrowserまでのPoint cold/warm/末尾改行後の回帰も検査する。利用者のホバー正常確認と、今回の色修正後のWindows実GUI確認を混同しない。

## Enterの独自動作の撤去

連続Enterで空行の字下げを1段戻す動作と、そのeditor履歴を削除した。通常のEnter・手動dedent・括弧内・Low・設定幅・undo/redoは維持する。統合版PY262で変更前の新期待4件が失敗し、撤去後は入力関連6 suite・81件が成功した。[変更した操作契約と以前の実装履歴](jetbrains-python-editing-plan.md)を参照する。

## 未確認範囲

ローカルPY262/JDK25（Java21 bytecode）検証用に固定した0.1.6候補ZIP（327,317 bytes、SHA-256 `f3eda744902905792d48ec19e85952974aa2f65f5097e390103371b955025439`）はPY262 Plugin VerifierでCompatibleだった。deprecated API3件・experimental API4件の警告を保持し、warning-freeとは扱わない。このSHAはローカル検証用ZIPの識別であり、IC251/JDK21で作るCI共通ZIPの配布checksumとは分ける。公開候補の正本は最新HEADのCI artifactとそのchecksumで確認する。置換途中取消・入力途中・listener直接例外の追加前に成功した全体291/296/304件や旧ZIPは中間証拠であり、最終sourceの検査へ代用しない。headlessでの実IDE pipeline検証は、利用者のWindows GUIでの連続した描画確認とは区別する。PyCharm Pythonと同じ全言語機能や全入力状態を提供したという主張ではない。

## 公開前のゲート

今回の利用者指定は、**最新HEADの必須CI・独立監査→Draft解除→利用者確認→マージ/正式公開**の順である。Windows描画の利用者確認は公開前ゲートとして記録し、CI成功と混同しない。0.1.6の正式changelogを準備するが、利用者確認前にマージ・tag・正式公開を実行しない。Nagi本体/VS Codeの版と公開0.1.5の配布物は変更しない。
