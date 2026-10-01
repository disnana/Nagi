# Queues and workers

The runtime test uses an ingress queue with capacity 64 and at most eight workers. Failures retry after 1/2 ms backoff and count as dead letters after the retry limit. The producer and every worker are joined before exit.

In `examples/queue.nagi`, four of 40 jobs fail permanently and 36 complete. The test runs real timers, channels, and workers rather than mock calls to an external queue.

High queue declarations, arbitrary handlers, durable retries, stored dead-letter contents, per-job timeouts, and recovery after restart are not implemented. Retries can repeat side effects. A future API must specify idempotency and delivery guarantees such as at-least-once.
