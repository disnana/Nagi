# HTTP

Highの@get/@post/@put/@deleteからAxum routingを生成します。HTTP/1.1、keep-alive、id path parameter、型付きquery parameter、request body、JSON responseを実装しています。

`examples/crud.nagi`の`/query?limit=5&name=tp-li`はi32とstrへdeserializeします。class引数はbodyから直接読みます。bodyを`view[bytes]`として借りるhandlerも使えます。

標準の試験用endpointは`/health`、5chunkの`/stream`、echo WebSocketの`/ws`です。middlewareでrequest処理を2秒に制限し、bodyとWebSocket messageの上限を1 MiBにしています。ResultエラーはHTTP statusとJSONへ変換します。DB/内部エラーの詳細はresponseへ出しません。

JSON responseはnative classからVec<u8>へencodeしてBodyに渡します。socketへ直接serialiseする経路や、大きいclassのJSON streamingは未実装です。chunk streamは実行できますが、汎用のHigh streaming構文はありません。

HTTP/2、TLS、認証、任意middlewareのHigh宣言、deployment、header/idle connection timeoutの詳細な制御は今後の範囲です。現在のserverはloopback専用の試作です。
