# 0.2.2: editor token再利用候補の棄却

main `9fe5e079eb23ed4aef515fabd19ed86dc67958f7`をbaselineに検討・実装・測定したが、
通常assistで遅延が再現したため、この最適化は不採用とした。compiler本体・候補依存testの差分は
すべて戻した。0.2.2の実装済み高速化として扱わない。残す成果は比較harnessと棄却結果である。

## 検討した設計

`Sources`はparserが使った元tokenを各物理fileごとに保持する。一方、symbolsのindex/import exportと
assistのquery/位置計算は同じtextを再lexする。ここをrequest内の不変slice借用へ変えた。

実parse modeをSourceFileへ保持し、既存consumerの拡張子由来modeと一致する場合のみ借用した。
public `source::load(path, high)`、任意拡張子nativeやsymlink先が不一致なら従来lexへfallbackした。
不完全入力のcursor/recovery用lex、overlay/project再load、High初回check、生成Lowの独立parse/check、
型・所有権・Policy検査、source/token番号・UTF-16位置・元診断を維持する設計だった。
generation/OS lock/旧exe/Cargo cache、release profile・依存・units対象は変更しなかった。

High/保存Low/手書きLow、import/native append、CRLF/Unicode、native `@replace`、overlay更新、
persistent request、partial/recovered入力、type/moveエラー、上限とprotocol failureを含む既存の
editor_assistance/module_symbols/symbolsとmode fallback追加testは成功した。

## 棄却判断

同じtoolchain/profileでbaselineを先にbuild・保存し、同一物理path・requestを交互の順番で測定した。
広い候補で500locals通常assistの悪化を確認後、hot pathのCowをsliceへ変えたが悪化が残った。
さらにeditor.rsをすべて戻し、symbols/index/import exportだけに狭めた最終候補を15sample測定した。

狭い候補でも通常assistはwall中央値128.957→136.639 ms、CPU中央値127.239→133.139 ms。
paired wall差は15/15で候補が遅かった。字句負荷やimportで短縮したcaseもあったが、通常caseの悪化を
相殺して採用しない。code layoutやallocatorは仮説で、根因は未特定。さらなる場当たり変更は行わない。

最適化本体と候補依存testを取り下げ、対象source/testとbaselineのdiffゼロを確認した。
[棄却patch・全raw sample・min/max・別memory観測](../../benchmarks/results/editor-token-reuse-2026-10-23/README.md)
を保存する。メモリはPython launcher由来RSSを分離する小さいLinux C probeによる別runを使った。
native実行速度、Cargo cache hit、CI全体や4 OSでの高速化は主張しない。

## 継続できること

[`benchmark_editor_queries.py`](../../scripts/benchmark_editor_queries.py)は同一pathのHigh/保存Low symbols、
assist queryなし/locals/member recovery/import、合成字句負荷を比較し、JSON全体の同値と独立した
候補・型・位置・診断assertを検査する。比較対象binaryとrequestのhash、wall/CPU/RSSのraw sampleと
中央値/min/maxを保存する。不利なcaseを除かず、今後の別案にも使える。

棄却後は別案として、symbolsの定義位置探索を当該物理lineへ絞る変更を検証した。
`windows(2).find`の全token走査を既存class fieldと同じ`partition_point`で限定する。
この別案はtoken借用・追加cacheを導入しない。[別の設計・測定・検証記録](../../benchmarks/results/editor-line-search-2026-10-23/README.md)
を参照し、本token再利用案の棄却結果と混同しない。
