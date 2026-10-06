# 並行処理

Nagiの非同期処理はTokio上で動きます。HTTPやタイマーを待っている間に、ほかの処理を進められます。使い方は[asyncとscope](async.md)から確認してください。

## CPU処理のために待ち時間をふさぎたくない

Pythonでも、`async def`の中で長い同期計算をすると、そのイベントループのほかの処理を遅らせます。`asyncio.to_thread(...)`などで処理を外へ渡す書き方があります。Nagiでは任意の関数を渡す同等のAPIは未対応です。

SQLiteは専用スレッドで処理します。任意のCPU処理は自動で移されません。次の`cpu_sum`は、Tokioのblocking workerへ渡す検証用APIです。

```nagi
async def main() -> Result[unit, Error]:
    result = try await cpu_sum(100000)
    print(result)
    return ok(print("完了"))
```

上の完全な例は、生成した整数列の合計を表示してから`完了`を表示します。`cpu_sum(100000)`の結果は`-3184`です。

`async def`に変えれば任意の同期計算も別スレッドへ移る、と考えるのがよくある間違いです。長い処理は分割して待ち合わせの機会を作るか、停止を管理できるRust連携や別プロセスを検討してください。

**一言でいうと：待つ処理と、CPUを使い続ける処理は別に扱う。** taskの開始と終了待ちは[asyncとscope](async.md)、長寿命workerの停止は[Supervisor](supervisor.md)にあります。

## CPU実行とデータの現行リファレンス

`cpu_sum`はCPU処理を分けて実行するためのサンプルです。同時に実行するCPU処理は最大4件です。利用者が任意の関数をこの枠へ渡すAPIは未対応です。

## データの受け渡し

複数のtaskへ、変更できる値を自動で共有することはありません。所有権を移して渡すか、`shared`で共有します。`shared`自体は読み取り用の参照で、内側の型が持つ独自の可変性をなくすものではありません。spawnへviewは直接渡せません。sharedで包むだけで、どんな値でもthread-safeになるわけではありません。共有handleの複製と中身のコピーも別です。Rust側の`Send`／`Sync`の要件は最終的にbuildで検証します。actorのmessage・replyではsharedも未対応です（[actorの制約](actor.md#容量とタイムアウト)）。

すでに始まったCPU処理は、呼び出し元をキャンセルしただけでは止まりません。長い処理には、その処理自身が停止の要求を確認する仕組みが必要です。

task数とHTTPの接続数は別です。待ち時間やメモリを含む測定結果は、[測定結果](../PERFORMANCE.md)と[通信の負荷試験](http-capacity.md)を参照してください。

Nagi独自のVMやCPU時間でtaskを強制的に切り替える仕組みはありません。yieldしない処理は他のtaskや停止処理を遅らせます。並行実行と共有データの安全性は、Nagiの検査に加えてRustの型検査とTokioの実行規則に依存します。実装は[並行処理のruntime](../runtime/src/concurrent.rs)にあります。
