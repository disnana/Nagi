# HTTP 413のCI観測と失敗診断

対象はmain base `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`。標準HTTPのruntime実装、公開契約、本文4096 bytesの上限、期限、依存、版を変更しない。quote-apiの元の4097 bytes本文送信と413期待、既存5件の`oversized_body_`回帰を維持する。

## 観測と判断

PR #104のhead `301ddc8` / tree `038022`で、[PR checks run 37974822219](https://github.com/disnana/Nagi/actions/runs/37974822219)のWindows job `113970438542`は保存Low quote-apiの`body-limit`で、413のstatusを読む前に`ConnectionAbortedError / WinError 10053`となった。同じheadの[push run 37974816405](https://github.com/disnana/Nagi/actions/runs/37974816405)のWindows job `113970429322`は成功した。旧失敗ログはchildを停止する前の状態やstdout/stderrを保存していないため、infra failureや新しいproduction regressionを確定できない。

[公開HTTP契約](../en/http-server.md#limits-and-shutdown)は過大本文の413選択・handler非呼出し・未読本文をdrainせずcloseする方針を示す。未読dataによるTCP resetと、OS/client/upload patternによって413受信を保証できない制限も明示する。[言語不変条件](language-invariants.md#http)も無制限drainを採用しない。Python `http.client`の`endheaders(body)`はheaderとbodyを別writeし、TCP_NODELAYを使う。従来nativeのTokio socket既定NODELAY=falseと即時writeが成功しても、同じpacket分割・受信を証明しない。

このため新native caseの413受信を全targetの必須成功条件へ昇格しない。NODELAY=trueでheaderと4097 bytes bodyを別writeし、8192 bytesまで・3秒以内のclose観測を保存する。EOFまたはreset/abortだけをcloseとして認め、送信側のbroken pipeも記録する。受信した完全status行は413、完全headerは`Connection: close`と本文長17、受信本文は`Payload Too Large`のprefixを厳密に検査する。完全受信・部分受信・resetを区別し、完全413受信の成功とtransport終了を混同しない。どの場合も過大本文handler呼出し0、次connectionの正常GET、connection/request capacity回復、server shutdownを要求する。

## 診断artifact

quote-apiは失敗case、送信・status/header読取・body読取・assert段階、停止前child PID/exit statusとstdout/stderrを`smoke-failure.json`へ記録する。logは各32768 bytesの末尾だけを読み、総byte数と切詰め有無を含める。失敗はそのまま再throwし、request再送、resetの成功扱い、413 assert緩和を追加しない。元のreadiness待機は維持する。

共通application runnerはproject・High/Low・tracebackとこの診断を`NAGI_FAILURE_DIR/application-quote-api-{high,low}.json`へ保存する。未指定時は既存CI対象`build/compiler-failures`を使う。診断入力を512 KiBまで、exceptionを4096文字まで、tracebackを16384文字までに限定し、保存失敗は元例外のnoteへ残す。

nativeは`NAGI_FAILURE_DIR/http413-nodelay-{pid}-{timestamp}.json`へ観測を保存する。fileをexclusive作成するため同じPID/clockでも上書きしない。環境変数未指定時はOS tempの`nagi-http-observations`を使う。CIの4 OS laneは`--nocapture`で観測を表示し、成功・失敗にかかわらず専用HTTP artifactをuploadする。application failureは既存failure artifactに含める。

## 有限な検査と次の判断

Linuxで既存5件＋新観測1件が成功した。新caseは完全413（201 bytes）、EOF、過大本文handler0、次GET/capacity回復を記録した。runtime libraryは232 passed / 0 failed / 1 ignored（既存費用測定）、runtime all-targets Clippy、workspace fmt、Python診断9件、CI policy59件が成功した。最初のPython診断runはsocket作成をsandboxが拒否したinfra failureであり、local socket accessを有効にしたrunと区別する。

High・保存Lowの既存executableを再利用し、元と同じ51 cases/形式が成功した。quote source/configとHTTP runtime sourceの一致、元smokeの全assert・check call・`endheaders(body)`のAST一致を確認した。この再利用はcompiler/Rustの新build成功ではない。新runtime testはwarm cacheでbuild/runした。原WindowsのresetをLinuxで再現したというRED証拠はない。

原ログ・hash・実行条件は[検査artifact](../../benchmarks/results/http413-ci-observation-2026-10-10/README.md)に残す。Windows・macOSでの変更後実行、最新headのCI、独立reviewは未確認。任意packet分割、全OS/clientへの413必達、無制限drainやhard wall期限を追加保証しない。

次はこの診断headを4 OS CIで観測する。現在の証拠だけではruntime修正を推奨しない。child正常・handler0・capacity回復のまま元smokeだけresetする場合は、公開制限と元fixtureの413受信期待との関係を明示したfixture判断を別変更で行う。handler呼出し、capacity未回復、期限外close等が観測された場合は、その失敗を固定したruntime回帰を先に作る。
