# Queueとworker

ランタイムの実試験は容量64のingressと最大8 workerを使います。失敗時には1/2 msのbackoff後に再試行し、上限でdead letter件数へ分類します。producerと全workerをjoinしてから終了します。

`examples/queue.nagi`では40 jobのうち4 jobが永久失敗し、36 jobが完了します。実際にtimer、channel、workerが動作する試験です。外部queueへのmock呼び出しではありません。

Highのqueue宣言、任意handler、durable retry、dead letter内容の保存、個別job timeout、再起動後の復元は未実装です。retryには副作用の重複があるため、将来のAPIはidempotencyとat-least-once等の保証を明示する必要があります。
