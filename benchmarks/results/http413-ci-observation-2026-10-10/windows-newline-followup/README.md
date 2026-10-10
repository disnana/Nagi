# HTTP診断テストのWindows改行修正（PR #105追加差分）

PR #105の固定head `f7d9a571ce65d094fe9e87633e85de1f99bf6b03`（local `55e54d2` と共通tree `8e673711…`）のchecks run `38029125681` / Windows job `114146219494` は、HTTP検証前のPythonテストで8成功・1失敗となった。偽の子プロセスが親のTextIOWrapperへ文字列を書き、WindowsでLFがCRLFへ変わったため、raw logのLF期待と一致しなかった。後続HTTP/application stepはskipで、failure artifactアップロードもファイル不在だった。元#104の実HTTP `WinError 10053` は別の未解決問題である。

修正は `scripts/test_application_native_tests.py` のみ。Popenが子へ渡すfile descriptorを模し、偽childはbufferへ明示LF bytesを出力する。元assertを維持し、stdout/stderrのbyte数、LFとCRLFの保持を追加確認した。runtime・smoke・HTTP入力・413期待・limit・CI workflowは変更していない。

| 検査 | 実結果 | 範囲 |
|---|---|---|
| 初回RED試行 | 1 error、socket PermissionError | 環境失敗。契約REDとして数えない |
| 元テスト＋CRLF writer強制 | 1 fail / 0 error | 実Windowsと同じLF/CRLF AssertionError。Linux上のTextIOWrapper模擬 |
| 修正後＋CRLF writer強制 | 10 pass / 0 fail/error/skip | raw bytesを保持。実Windows実行ではない |
| 修正後の通常Linux | 10 pass / 0 fail/error/skip | 小さい所有loopback fixture、既存例のmock検査 |
| 独立Sol High review | 修正必須所見0 | 差分・元assert・保存原ログ/hashを読取り照合。再実行なし |

`fix/` と `review/` は各ローカル原本のexact copy。`ci/` は元調査14 file中、report/sourcehash/原job log/元と今回のfailure excerptの5原本だけをexact copyした。元sourcehashの未コピーtool-result等は元ローカルpacketの参照であり、ここに全14 fileがあるとは主張しない。コピー対応は `provenance.json` に保存する。原資料の失敗・改行・末尾空白を編集していない。

実Windowsの修正後CI、HTTP原異常終了のclosure、最新4 OSは未確認。既存6 native HTTP観測・Linux runtime232成功/既存ignore1は元55e54検証で、このtest-only追加差分で再実行した件数ではない。merge・版更新・releaseは行っていない。
