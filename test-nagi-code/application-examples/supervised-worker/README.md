# Supervisorでworkerを分けて管理する

小さな梱包ジョブと監査カウンターを別のactorで処理し、常駐connectorを`actor.task_with_ready`で監督するCLIです。起動すると決まったジョブと故障を一度ずつ実行し、検証して停止します。ネットワークやDB、入力操作は不要です。

このディレクトリで実行します。Nagi 0.1.9とRust/Cargoが必要です。

```sh
nagic check
nagic run
```

次の順で確認します。

1. 梱包5件、監査7件を受け付ける。
2. 梱包件数`-1`を業務エラー`JobError.InvalidUnits`として返す。梱包状態は5件のままで、再起動しない。
3. `Job.FailWorker`でhandler自体が`Error`を返す。呼び出しは`REPLY_LOST`になり、Supervisorは梱包actorを第2世代へ再起動する。factoryが初期状態を作り直すため0件に戻る。
4. 監査actorはそのまま2件を追加し、合計9件を保つ。梱包の再起動を待ってから4件を処理する。
5. connectorの第1世代のpanicと第2世代の稼働を確認する。最後に`shutdown`し、connectorの生存数0、Dropによる後片付け2回、子3つの停止、イベント列の終了を確認する。

connectorは並行に起動するため、実際のpanicと梱包ジョブの時間的な前後関係は固定しません。梱包の再起動中に監査が何ミリ秒動けたかは測りません。監査が一度だけ起動し、再起動の前後で状態を保って処理したことを検証します。

stdoutには3行の経過とJSONを出力します。

```text
job rejected: packing total stays 5
packing restarted: total reset to 0; audit continued at 9
connector panic recovered: generation 2 is active
```

JSONの主な値は`packing_total: 4`、`audit_total: 9`、`failed_events: 1`、`panicked_events: 1`、`restarts: 2`、`connector_active: 0`、`connector_cleanups: 2`です。Rustのpanic hookは捕捉されたpanicもstderrへ出します。`supervised-worker: controlled connector panic`はこの例で意図した診断で、プロセスは成功時に終了コード0で終わります。

## 失敗を返す位置を分ける

| 例の失敗 | 戻り値・観測 | 意味 |
| --- | --- | --- |
| 件数が範囲外 | `Ok(Turn(state, Err(JobError)))` | 業務上の拒否。状態を保存し、workerを続ける |
| 梱包workerの故障 | handlerの`Err(Error)`、`FAILED`、`REPLY_LOST` | actor自体の失敗。TRANSIENT方針で再起動する |
| connectorのpanic | taskの`PANICKED` | 正常な業務返信ではない。TRANSIENT方針で新しいtaskを起動する |
| 停止後の呼び出し | `Err(CallError)`、kindは`CallKind.STOPPED` | 処理を受け付けない |

状態や未処理ジョブは永続化していません。再起動は保存・ロールバック・再送ではありません。実業務で使う場合は、factoryで永続状態を復元し、ジョブIDと結果確認で重複実行を防ぐ構成が必要です。

## NagiとRustの担当

[main.nagi](main.nagi)がactor、メッセージ、業務エラー、再起動の確認、イベント集計、明示的な停止を実装します。[native.rs](native.rs)は検証用connectorだけです。最初の呼び出しでpanicし、次の呼び出しではキャンセル可能なfutureを待ちます。Rustのatomicで試行数と生存数を測り、futureのDropで後片付けを数えます。追加crateや別のTokio runtimeは使いません。外部システムへ接続するconnectorではありません。

`shared[Context]`は共有所有を表し、Nagiで変更可能な共有カウンターを提供するものではありません。一度だけのpanicとDropの観測に必要な可変状態は、この小さなRust adapterへ置いています。これをNagiの実装済み機能としては扱いません。

すべてのControlは`run`の前に作ります。`clone_control`は作成時点から独立にイベントを読み、過去の読み取り位置を複製しません。梱包の再起動、connectorのREADY、最終集計にはそれぞれ独立したControlを使い、先に読んだイベントを失わず集計できます。固定sleepを成功判定には使いません。actorの起動・呼び出しと各イベント待ちには5秒の期限があり、`WaitKind.TIMEOUT`を列の終了と区別します。イベント待ちを含めたプロセス全体は[smoke.py](smoke.py)が15秒で停止します。

`finish`は業務検証の結果を保存してから`shutdown`を待ちます。検証が`Error`を返しても停止を試みます。成功した`shutdown`とscope終了で、監督下の子と共有データの後片付け完了を確認します。これは同一プロセス内のTokio taskを監督する実装です。VM、無停止のコード差し替え、分散配置、永続mailboxは未対応です。

## 準備完了と次の実用例

`STARTED`はtaskの起動を示し、初期化の完了を保証しません。connectorは後片付け用guardを作ってから`mark_ready`を呼び、第2世代の`READY`を一度だけ発行します。Nagiは`next_event_timeout`でそのイベントを待ちます。稼働判定のためにnativeカウンターを繰り返し調べる必要はなくなり、カウンターは後片付けの検証に使います。

[actorの書き方](../../../docs/actor.md) · [Supervisor](../../../docs/supervisor.md) · [English](README.en.md)
