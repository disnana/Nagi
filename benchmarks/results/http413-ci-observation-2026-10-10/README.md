# HTTP 413のCI診断検査

main base `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`からの診断/test-only変更。[方針・契約・未確認範囲](../../../docs/internal/http413-ci-observation.md)を参照する。

| 実行 | 証拠 | 結果 |
|---|---|---|
| 既存5件＋NODELAY別write観測 | [初回native](native-first.log) | 6 passed / 0 failed / 0 ignored |
| 最終runtime library | [原log](runtime-lib-final.log) / [最終観測](native-final.json) | 232 passed / 0 failed / 1既存ignored。完全413・handler0・capacity回復 |
| runtime全target Clippy | [log](clippy-final.log) | 成功。`-D warnings` |
| Python失敗診断回帰 | [log](python-capture-final.log) | 9 passed。例外保持・停止前live child・bounded tail・CI保存先 |
| CI policy回帰 | [log](ci-policy-final.log) | 59 passed |
| High/保存Lowの元smoke | [結果とbinary hash](quote-reused-binaries.json) / [実行script](verify_existing_quote.py) | 各51 cases成功。既存binary再利用、新buildではない |

runtimeは`cargo test --offline --locked -p nagi-runtime --lib -- --test-threads=1`、Clippyは`cargo clippy --offline --locked -p nagi-runtime --all-targets -- -D warnings`。workspace fmtも成功した。Rust cache・debug条件、選択artifact hash、元Windows full logの保存先/hashは[validation](validation.json)に残す。runtime logのignoredは既存の費用測定で、新観測のskipではない。

repository内のruntime logコピーは末尾の空行だけを除いた。workspace内のraw logを維持し、そのhashと保存先もvalidationへ記録した。

旧Windows run `37974822219` / job `113970438542`は保存Lowの413 status受信前にWinError10053、同じheadのpush run `37974816405` / job `113970429322`は成功。raw Windows logはworkspaceの既存証跡directoryに保持し、全334 KiBをこのtest変更へコピーしていない。初回Python回帰はsandboxのsocket作成EPERMで、後のsocket許可runと区別する。

元smokeの全assert、`check(...)` call、`endheaders(body)`はbaseとAST一致。request再送、resetの成功扱い、公開保証/runtime/依存/版の変更はない。Linux上では元Windows失敗を再現しておらず、native REDはない。全workspace/compiler/native application rebuild、Windows/macOS、変更headのremote CI、独立reviewは未実施。
