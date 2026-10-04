# Supervisorでカウンターを管理する

actorが値を順番に更新し、HTTP handlerが結果を返す例です。`CounterError`は利用者向けの失敗で、値を保ったまま返します。actor自体が失敗するとSupervisorが再起動し、factoryで初期状態を作り直します。

このディレクトリで実行してください。

```sh
nagic check
nagic run
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

SupervisorとHTTPは同じscopeに起動します。Supervisorが最終的な失敗を返すと、scopeがHTTPも取り消します。正常な`/shutdown`はこの失敗経路とは別です。実行基盤は同一プロセス内のTokio taskです。

状態はメモリ上にあり、プロセス停止やactorの再起動で初期値に戻ります。再起動は保存やロールバックの代わりにはなりません。これはローカルで動作を試す例で、公開用の認証は付けていません。

[English](README.en.md)
