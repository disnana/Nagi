# SQLite設定API

SQLiteの設定を型付きclassとして読み、JSONで返します。NULL、bool、f64、文字列、BLOBを含む3件のデータを起動時に用意します。既存の行は上書きしません。

リポジトリの最新ソースから作った`nagic`を使ってください。0.1.8の配布版は、この例の`bool?`、`f64?`、`bytes?`をSQLiteの行として読み取れません。

```sh
nagic run --project test-nagi-code/application-examples/device-settings/nagi.toml
```

```sh
curl http://127.0.0.1:8091/devices
curl http://127.0.0.1:8091/devices/1
```

`GET /devices`は全件、`GET /devices/{id}`は1件を返します。未登録IDは404、整数でないIDは400です。BLOBはJSONの整数配列になり、SQLのNULLはJSONの`null`になります。

接続先はローカルホストです。`NAGI_SAMPLE_PORT`でポート、`NAGI_SAMPLE_DB`でSQLiteファイルを指定できます。既定値は8091と、実行時のカレントディレクトリに置く`settings.sqlite`です。

これは認証なしの読み取り専用サンプルです。設定更新APIや複数操作をまとめるトランザクションは含みません。

`smoke.py`はHTTPの値と型を確認し、サーバーを停止してSQLiteの1行を更新したあと、同じファイルで再起動します。起動処理が保存済みデータを上書きしないことと、停止後にポートが解放されることも確認します。共通の検証コマンドは[上のREADME](../README.md)を参照してください。
