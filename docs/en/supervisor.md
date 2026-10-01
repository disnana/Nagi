# Supervisors and failures

The runtime experiments with one_for_one worker restarts. A child task panics, its JoinError is detected, and that worker is recreated. Tests also confirm that another worker continues.

Restarts are limited within a one-second window. When a crash loop exceeds the limit, restarts stop. Restart latency is measured from detection until the new child task first runs. Backoff is 1 ms.

`examples/supervisor.nagi` deliberately causes three panics. Panic messages on stderr are part of the test. Normal shutdown also aborts and joins independent workers.

High supervisor-tree declarations, arbitrary child factories, one_for_all/rest_for_one, and HTTP request redelivery are not implemented. This worker test does not measure request loss. Handling Rust panics does not isolate SIGSEGV, aborts, or OS process termination within the same process.
