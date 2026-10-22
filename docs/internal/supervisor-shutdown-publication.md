# Supervisorのterminal event publication（2026-10-22）

対象はmain `f75300abcbc1c681832d445331107224b5f5307e` と同一の専有worktree。公開API・fixtureのイベント件数、期限、test並列度は変えない。0.2.1候補CI run39779857773のWindows packageではsupervised-worker High nativeの `stopped == 3 && shutdown == 1` が失敗したが、実数が出ていないため以下のraceと同根だとは断定しない。

## 原因と採用済み修正

最初の `request_stop` はstop flagを立て、ledger内の子をabortした後、ledger lock外でSHUTDOWNを送る。一方run drainerはstopを見て子をjoinし、context cleanupを終えると `wait_terminal` でCompleteへ進み、event senderを閉じる。最初のstop requesterがpublication前で止まると、その通知を送る前にchannelが閉じる。STOPPEDは同じdrainerがjoin処理の後に送ってからterminalへ進むため、この通知欠落と同じ原因ではない。

Groupへ単調な `shutdown_published: AtomicBool` をfalseで追加し、最初のrequesterがSHUTDOWNを送った直後・既存notify前にRelease storeする。Complete/sender closeの条件はAcquire loadも要求する。stopの取消開始、cleanup開始、join、STOPPEDの送信、既存lock orderは変更しない。新task・timer・sleep・retryは追加しない。

## 決定的なRED→GREEN

cfg(test)のinstance限定FnOnce hookを、abort/ledger unlock後・SHUTDOWN前に置く。既存の有限Gateでstop threadだけを止め、test自身がrun futureをpollする。既存snapshotで子recordとcleanup recordがなくcontextが破棄された瞬間のPollを保存する。gate解除、stop threadの終了/join、run完了、イベントdrainを済ませてから、保存したPollがPendingでSHUTDOWN/STOPPED各1件であることをassertする。旧Ready経路でも最後のassert前に片付ける。

hookだけを足した旧実装は `run completed before SHUTDOWN publication: shutdown=0, stopped=1` でRED（test実行0.00秒、初回compile1分46秒）。publication barrier追加後は同testがGREEN（test実行0.00秒、compile込み41.26秒）、既存actor 45件も0.20秒成功した。試行の反復・確率的sleep・fixture件数変更で再現させたものではない。

## 検証と証拠

既存release-only native cache 524MiBを、exclusive作成した `/tmp/nagi-supervisor-events-*` の専有targetへcopyした。Cargo自身が全fingerprintを再判定し、coldや速度比較とは扱わない。root debug targetは利用しない。`CARGO_BUILD_JOBS=2` はcompile容量管理用で、testの通常並列度は変更しない。workspace2GiB/tmp1GiBの容量guardを置き、元CLI・既存cache・logsは整理しない。

raw RED/GREEN、actor/full-runtime/clippy/fmt、High/saved Low/native結果とsource/evidence manifestはtracked外 `local-only/supervisor-events/` に保存する。最終結果は、`cargo test --locked -p nagi-runtime` が33.06秒で成功（lib863 passed/1既存ignored、integration/docを合わせ923 passed/3既存ignored、failure0）。runtime all-targets clippy `-D warnings` は51.57秒、fmtも成功した。既存のsupervised-worker harnessは28.54秒でHigh/check/build/nativeと独立saved Low/check/build/nativeの両方が成功し、各12 fieldsを完全一致で確認した（STOPPED3/SHUTDOWN1、panic/restart/context cleanupを含む）。build logは修正worktreeのruntimeが実際に再compileされたことを示す。compilerは既存の正式0.2.1 Linux CI配布候補を再利用し、compiler source自体は変更/buildしない。

source/testはこの時点で凍結。独立auditがCargoを再利用できるよう専有targetを保持して親へ所有権を返し、audit後はこのtargetだけを整理できる。生成out/実行exe/log/resultsは別途evidence archiveに保護する。独立High最終auditと改訂0.2.1候補の4OS CIは別gate。Windowsで実数を観測していない事実は維持する。
