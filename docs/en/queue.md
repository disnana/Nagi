# Queue and worker experiments

A queue passes accepted jobs to workers. This page describes the fixed `queue_demo` test API, rather than a general-purpose job queue. It uses Tokio channels and tasks, with a queue of up to 64 jobs and at most eight workers.

In [queue.nagi](../../examples/queue.nagi), 36 of 40 jobs complete and four still fail after retries. Retries wait one and then two milliseconds. Jobs that reach the retry limit are counted as failed. Shutdown waits for both the producer and the workers.

Submitting arbitrary user work, persisting jobs and retries, and recovering them after a process restart are not supported. Retries can execute the same work more than once. A general-purpose API still needs rules for handling duplicate execution.

See the [concurrency runtime](../../runtime/src/concurrent.rs) for implementation and tests. [Actors](actor.md) let you define your own business messages and handlers.
