# CI検証レベル（0.2.0以後の候補）

0.2.0の凍結済みreleaseとは分離する。高速化の実測結果はまだない。既存の包括検証commandと診断parserはそのまま保持し、実行条件とOS matrixだけを変更する。新しい配線を逆変換すると従来workflowのbytesへ完全に戻り、従来のsource guardを適用できる。

| 検証 | L1 通常PR | L2 main・週次・昇格PR | L3 RC・正式release |
| --- | --- | --- | --- |
| 変更検出・source guard・release計画・policy | 必須 | 必須 | 必須 |
| Linuxのcompiler/runtime lib、固定2個のfrontend integration、fmt/clippy、doctest | code変更で必須 | 包括検証に含む | 包括検証に含む |
| 従来Linux包括検証・Rust 1.88 | 後段で実行 | Nagi対象時に必須 | Nagi対象時に必須 |
| 従来package・CLI/generated/native regression | 後段で実行 | Linux・Windows・macOS ARM | Intelを含む4 OS |
| browser trust/E2E | 従来のPR選択を維持 | Nagi対象時に必須 | Nagi対象時に必須 |
| IDE・VSIX/plugin package | 従来component条件 | 従来component条件 | release componentごとに必須 |
| install・upgrade・uninstall・展開・security・互換性 | 包括段階で実行 | 選択OSの元command | Nagi対象時に全4 OSの元command |
| 固定名Ready aggregate | 必須 | 必須 | 必須 |

L1のRust pathは`compiler/src/lexer.rs`・`compiler/src/parser.rs`・`compiler/tests/enum_parser.rs`だけに限定し、既存のpresentation/plugin pathも維持する。checker・emitter・lowering・全runtime/task/ownership/auth/storage/network境界・依存・build・workflow・未知pathはL2へ昇格する。Git rangeの欠落、不正、空、不完全な比較も昇格する。空filterや任意のthread制限は導入しない。critical inventory guardは固定4 targetをcompileし、各実行fileに重複のない非空test一覧があることを実行前に確認する。

検証済みrelease計画のcomponentが1つでもrelease対象ならL3にする。手動のrelease coverage指定もL3にするが、公開許可にはならない。IDEだけのreleaseでNagi検証を新たに要求しない。flag・matrix・boolean・選択job結果を照合し、必須jobのskip・欠落・cancel・failureをReady成功にしない。publishはReady・ci-plan・release-plan成功、L3、main push、実際の承認済みreleaseを要求する。公開前に選択jobとsource SHAを再検査する。

週次は月曜03:23 UTC。version/CHANGELOG承認は既存release plannerに残す。PR・tag・手動実行から自動公開しない。mainのrelease実行を後続main commitでcancelしない。

nextestは任意比較段階であり必須test runnerへ置換しない。testごとにprocessが分かれるため、入れ子Cargo/native build、global state、resource競合、libtest出力依存parserは個別の証拠が必要。doctestはCargoで保持する。release/test profile、timeout、assert、seed、featureは変更しない。cache・action pin・publish最終配線も本差分の独立review対象とする。

公開前には `publication_gate.py` が全選択job、release/package整合、matrix、workflow commitとrelease-plan SHAの一致を再検査する。選択jobの未完了・失敗・取消・skipでは公開しない。キャッシュ・依存ツールの比較と測定の限界は[監査記録](ci-optimization.md)、nextestは[評価手順](nextest-evaluation.md)を参照。
