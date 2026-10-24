# 0.2.2向けSCC分割粒度の比較（不採用）

main `235a76d9`の関数単位ビルドと、約64 SCCを1 crateへまとめる候補を比較した。100関数では8→2 unitとなり、449 SCC以上では旧方式と同じ構成になる。候補範囲・SCC・依存順・内容照合・Cargo判定・世代保護は変えず、分割数だけを変更した。

**候補は取り下げた。** 初回の大きい短縮は追加測定で再現せず、改善と悪化が混在した。閾値を探し直すことはせず、現在のopt-in分割方式と既定releaseを維持する。このPRによる利用者アプリのビルド高速化は主張しない。

## 条件と結果

Linux、rustc/Cargo 1.99.1、同じmainから作ったrelease compiler、実runtime、変更していないdev profile、Cargo jobs=2、offline。同じ物理ソースと同じseed lock、版ごとの専有out/targetを使った。初回は空targetから旧→候補の順に実行し、追加比較では両版をwarmにして各caseを同一入力10組、AB/BA交互に実行した。2つの合成fixtureを区別し、閾値は64に固定した。

時間はms。変化率は10組の「候補/旧版」の中央値で、独立した中央値同士の比率とは異なる。負数は短縮。

| fixture | case | 旧版中央値 | 候補中央値 | paired変化 | 候補が速い組/10 |
|---|---|---:|---:|---:|---:|
| aggregate | 変更なし・stats有り | 192.77 | 188.20 | -3.04% | 8 |
| aggregate | 変更なし・stats無し | 179.34 | 174.81 | +0.11% | 5 |
| aggregate | 本体編集 | 256.62 | 251.43 | -1.42% | 9 |
| aggregate | 公開API編集 | 259.49 | 258.52 | -1.09% | 7 |
| print-each | 変更なし・stats有り | 196.27 | 202.69 | +2.47% | 4 |
| print-each | 変更なし・stats無し | 189.49 | 184.18 | -2.53% | 7 |
| print-each | 本体編集 | 256.60 | 254.75 | -1.63% | 6 |
| print-each | 公開API編集 | 263.08 | 259.51 | -3.19% | 7 |

初回aggregateのAPI編集は各5回の中央値330.56→249.11ms（-24.6%）だったが、旧版の後半でwall/phase時間が一時的に増え、追加の交互比較では再現しなかった。原因は断定しない。初回cleanは66.447→64.530秒だが各1回・registry/OS cacheはwarmなので、clean改善の根拠にはしない。

CPUのpaired変化もcaseにより-3.75%から+2.59%に分かれ、peak child RSS中央値の差は全caseで1%未満。メモリ改善は主張しない。CPUは子孫processを含み、並列処理でwallを超え得る。RSSは単一childのhigh-water markであり同時使用量の総和ではない。初回のlogical targetサイズは旧1,337,006,347 / 候補1,340,442,746 bytesで、容量削減も確認できなかった。

## 正しさと限界

初回44個＋交互比較164個のnative出力を最終再実行し、記録されたSHA・利用可能な158 receiptsを検証した。stats有りの全buildでfull checks・Cargo常時実行・native再利用のCargo委譲・unit数8/2を確認した。stats無しbuildはreceiptを出さない。warmの全unit Freshを確認し、編集時のFresh/Dirty・phase・CPU/RSSは全sampleに保存した。

追加比較でも履歴は増える。実行順は均等だが、統計的な有意差・一般的な速度向上・実アプリ・IDE・Windows/macOS・production実行性能は証明していない。採用gateを満たさなかったため1000関数、8ケースの全面matrix、製品用fixture変更には進めなかった。本体変更は取り下げ済みで、検査の省略や期待値の緩和は行っていない。

専有Cargo target 2個のみ整理して約3.14GBを回収した。ソース、CAS、全世代・実行ファイル、compiler binary、raw logは開発環境に保持。このPRには集計を掲載し、binaryや生ログarchiveは含めない。

## 保存物と再現

- [summary.csv](summary.csv): 全caseのwall/CPU/RSS中央値・min/max・paired方向。
- 非公開の作業記録: 初回JSON/log、全交互比較sample、compiler provenance、不採用patch、一時harness、検証・整理記録を保持。archive内637 fileのSHA-256は独立レビューでも照合済み。公開された集計だけからraw全体を再検証できるとは主張しない。

初回は`benchmark_incremental_builds.py --baseline-mode units --current-mode units --fixture aggregate --functions 100 --repeat 5 --edit-repeat 5`を使用。baseのcompilerと、`compiler/src/function_units.rs`のwidth計算だけを次に変更したcompilerを同一toolchain/profileで用意し、同じruntime-rootを指定する。

```rust
let target_units = sccs.len().div_ceil(64).min(8);
let width = sccs.len().div_ceil(target_units);
```

追加比較では初回のprojectと版別out/targetを使い、aggregateとprint-each各fixtureへ切り替えた後に両版をprimeする。各caseで同じ入力を両版へ渡し、順序を10組AB/BA交互にする。bodyは`(i*7)%100`の関数の値を毎回変更し、APIは`(i*7+3)%100`の戻り値を一度ずつi64→i32へ変更する。aggregate側の呼び出しにはi64への変換を追加する。値・出力・receiptを照合し、wall/CPU/RSSとFresh/Dirtyを保存する。別環境では新しい出力先へ自分の初回結果を作り、当時の証拠を上書きしない。cleanup済みのtargetをwarmと扱わない。
