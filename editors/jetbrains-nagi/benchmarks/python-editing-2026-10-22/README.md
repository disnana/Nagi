# JetBrains入力補助の検証・観測

2026-10-22 JST。基準main `5d3e58e`、IntelliJ IDEA 2025.1.1/build 251.25410.109、Linux x86_64、SDK付属JBR 21.0.6、Java 21 target、Gradle 9.4.0。同一compiler treeから既にbuildしたnagicを指定した。

`NAGI_TEST_COMPILER=<nagic> ./gradlew --no-daemon test buildPlugin`で実IDE actionとcompiler連携を含む全186件が成功し、失敗・skipは0。ZIP生成と`scripts/releases/jetbrains.py`によるdescriptor/hash検証も成功した。ZIPは開発中の候補で、正式公開や版更新の証拠ではない。

## 入力時間

`NagiInputResponsivenessTest`が実IDE actionを3回warmup後、20回計測した。fixture作成と各回の末尾復元は計測外。正しさは固定期待値で検査し、時間の閾値で成功・失敗を変えない。[raw](responsiveness.txt)と[環境・source/hash・件数](summary.json)を保存した。

| 操作 | 行数 | 中央値 ms | p95 ms |
|---|---:|---:|---:|
| Enter | 20 | 5.288 | 5.889 |
| `(` | 20 | 2.768 | 3.564 |
| `else:`のコロン | 20 | 3.935 | 4.257 |
| 閉じ`)` | 20 | 4.047 | 4.780 |
| Enter | 10,000 | 5.199 | 5.844 |
| `(` | 10,000 | 11.495 | 30.293 |
| `else:`のコロン | 10,000 | 2.669 | 2.861 |
| 閉じ`)` | 10,000 | 5.018 | 7.431 |

10,000行のtyped補正では、上限により**補正せず通常入力へfallbackする**ことを検証している。大きなファイルでも全てのsmart編集を提供できた測定ではない。変更前の同条件比較、描画、IME、人間による編集全体の所要時間は測っていない。高速化率や応答時間保証は主張しない。

## 回帰と未確認範囲

- 実選択範囲の置換で、IDEが`beforeCharTyped`より前に選択を除去するケースを検出した。選択解除前のcaptureにより、コロン/閉じ括弧による置換時の不要な整列を防いだ。
- Undo/Redo後に古いEnter gestureが再利用されるケースを単独テストで再現した。stampに加えて変更sequenceを比較し、同じstampが復元されても再利用を拒否した。両方を追加回帰と全186件で再確認した。
- 初回は実行JREにCLIのjavacがなく停止した。SDK付属の実JBR compilerを指定し、selected/source/target/release 21の検査を維持した。ZIP生成時のIDEデータ保存先はread-onlyだったため、XDGの保存先を作業領域へ設定した。skip・期限延長・assert緩和で通していない。
- `buildSearchableOptions`のIDE背景ネットワーク処理には接続警告があった。build/ZIP検証は成功したが、警告なしの実行とは扱わない。
- 独立監査・IDEA/PyCharm stable/EAPのexact-head CI・Plugin VerifierはPR本文で別記する。このローカル実行はPyCharm Python自身の動作比較や実GUI/IME受入の代わりではない。

調査対象と次の実装順は[設計記録](../../../../docs/internal/jetbrains-python-editing-plan.md)を参照。
