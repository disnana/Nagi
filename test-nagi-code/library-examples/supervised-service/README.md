# Supervisorでカウンターを管理する

actorが値を順番に更新し、HTTP handlerが結果を返す例です。`CounterError`は利用者向けの失敗で、値を保ったまま返します。actor自体が失敗するとSupervisorが再起動し、factoryで初期状態を作り直します。この例のSupervisor monitor移行は既存Task APIで実装済みです。Nagi 0.1.11で公開済みです。利用するcompilerが0.1.11以降であることを確認してください。

このディレクトリで実行してください。

```sh
nagic check
nagic run
nagic run main.low
```

別のターミナルで試せます。Windowsでは必要に応じて`curl.exe`を使ってください。

```sh
curl http://127.0.0.1:8090/counter
curl -H 'Content-Type: application/json' -d '5' http://127.0.0.1:8090/counter
curl -i -H 'Content-Type: application/json' -d '-1' http://127.0.0.1:8090/counter
curl http://127.0.0.1:8090/counter
curl -i -X POST http://127.0.0.1:8090/shutdown
```

順に`0`、`5`、409、`5`、204を返します。加算は1〜1000、合計は1000000までです。JSONが整数でなければ400、actorの受付が満杯・停止中なら503です。更新は自動で再送しません。タイムアウト後も加算済みの場合があります。このAPIには要求IDによる重複排除がなく、現在値だけでは個々の加算が成功したかを判定できません。

`/shutdown`はactorを停止します。HTTPはその後も起動しており、カウンターの要求には503を返します。HTTPの停止はCtrl+Cです。ポートは`NAGI_SAMPLE_PORT`で変えられます。

SupervisorとHTTPは同じscopeに起動します。`monitor_task = spawn monitor(group)`は`Task[Result[unit, Error]]`を作り、`await monitor_task`が`Result[Result[unit, Error], TaskFailure]`を返します。親は`await monitor_task`の`Ok(inner)`を`try inner`で処理し、Supervisorのterminal Errをbody Errとして伝えます。これによりHTTPへ取消を要求し、scopeの直接の子を実joinしてから元Errorを返します。HTTPは旧statement spawnを維持し、HTTP自体のErrもscope故障になります。正常な`/shutdown`ではinnerはOkとなり、HTTPを止めません。TaskFailureを表示してもscope故障は残ります。

`discard(monitor_task)`への置換は受取放棄なので、内側のterminal ErrによるHTTP停止を失います。HTTP stateにはActorとControlだけを持たせ、Supervisor contextを保持して終了待ちと解放待ちを循環させない構成です。親Futureの同期Dropは取消要求までで、非同期の終了確認や任意のHTTP handlerのclose完了を保証しません。実行基盤は同一プロセス内のTokio taskです。

状態はメモリ上にあり、プロセス停止やactorの再起動で初期値に戻ります。再起動は保存やロールバックの代わりにはなりません。これはローカルで動作を試す例で、公開用の認証は付けていません。

[English](README.en.md)
