# ADR 009: AxumサンプルでContent-Type欠落時の本文を期限付きで読む

状態: 2026-10-06にユーザーが案Aを承認。実装と修正後CIはこれから確認する。対象は`axum-service`サンプルだけ。

## 問題と根拠

PR #80のhead `0b2a5a5`のWindows CIで、通常のPOSTに対する415を読む前にWinError 10053を観測した。Axum 0.8.9のJsonはContent-Type不適合を本文読取前に拒否し、Hyper 1.11.1は未読本文を一度pollして残っていればreadを閉じる。通常のPython clientはheaderとbodyを別々に送る。

Linuxの固定観測では本文未送信でも415とEOFまで到達した。早期closeの機構は確認したが、Windowsの10053の直接原因まで再現した証拠ではない。元requestを一括送信・空本文へ変えたり、例外を成功扱いしてCIを通したりしない。

## 採用

- `POST /quotes`のContent-Typeが欠落している場合だけ、Axumのsafe `to_bytes`で本文を読む。data bytes上限は既存の4096、期限は読取開始から1秒。chunkごとに延長しない。
- EOF、上限超過、読取error、期限切れのいずれでも、元のAxum `MissingJsonContentType`の415と本文を優先する。EOFまで確認できなかった場合は`Connection: close`を明示する。
- headerがある場合は従来のJson extractorへ委譲する。Serde、業務Result、正常200と400/422/413の判断は維持する。
- Tokioのcooperative timeoutを使い、背景drain taskや追加依存・feature・unsafeを導入しない。期限切れでは読取FutureとbodyをDropする。

1秒はこのサンプルの選択で、Nagi標準HTTPの既定値ではない。上限4096から導いた値でもない。拒否時には最大4096 data bytesのbuffer・copyと待機を追加する。normal branchに読取やcloneを追加しなくても、handler Futureの大きさや全体費用が不変とは限らない。

## 保証しないこと

415の選択とclientへの必達は別。上限・読取error・期限超過で未読bodyが残れば、応答前後に接続が切れる可能性は残る。data bytes上限はwire・trailers・Hyperの総メモリ上限ではない。

CPUを占有するpollのhard wall期限、header受信・正常JSONの受信・handler全体・shutdown期限を追加したとはしない。存在する不正／非JSON Content-Type、Expect、任意TCP分割、keep-alive再利用はこの修正の保証範囲外。

## 不採用

全POSTの事前bufferは、正常経路のallocationと期限、415と413/400/408の優先を広く変えるため採らない。clientの一括sendは合法な分割送信の故障条件を除くため採らない。Axum/Hyper本体のpatch、独自HTTP再実装も行わない。

## 検査と移行

設計記録→先行回帰→修正の順。EOF・4096/4097・読取error・遅延EOF・期限・読取Futureの取消/Dropを検査する。元のHTTPConnection.requestと13バイトPOST、全status/body期待を維持する。High・保存Lowの実HTTPと、両方から生成されたnative unitを4 OSで実行する。sleep経過だけを証拠にせず、制御Futureのpoll/完了を観測する。

新helperがない旧版で先行testのcompileに失敗した場合は、その事実とWindowsの元failureを分けて記録する。compile失敗を10053の決定的再現とは呼ばない。成功するまでのretry・skipは加えない。compiler/resource/runtimeとgoldenの期待は維持し、修正後のCI完了まで#80をdraftに保つ。

根拠: [Axum Json](https://docs.rs/axum/0.8.9/src/axum/json.rs.html)、[Axum body](https://docs.rs/axum/0.8.9/src/axum/body/mod.rs.html)、[Hyper connection](https://docs.rs/hyper/1.11.1/src/hyper/proto/h1/conn.rs.html)、[Tokio timeout](https://docs.rs/tokio/1.53.1/tokio/time/fn.timeout.html)、[Python client](https://github.com/python/cpython/blob/v3.12.10/Lib/http/client.py)、[CIの失敗記録](../resource-contract-results.md#集約後ci-windowsのaxumサンプルで停止)。
