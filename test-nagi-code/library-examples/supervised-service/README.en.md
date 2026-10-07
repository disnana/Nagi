# Manage a counter with a Supervisor

An actor updates a value in order, and HTTP handlers return its replies. `CounterError` represents a business failure and preserves the current value. An actor failure lets the Supervisor restart it; the factory creates fresh initial state. The monitor migration in this example uses the existing Task API and is published in Nagi 0.1.11. Confirm that the installed compiler is version 0.1.11 or later.

Run from this directory:

```sh
nagic check
nagic run
nagic run main.low
```

Try these in another terminal. Use `curl.exe` on Windows if needed.

```sh
curl http://127.0.0.1:8090/counter
curl -H 'Content-Type: application/json' -d '5' http://127.0.0.1:8090/counter
curl -i -H 'Content-Type: application/json' -d '-1' http://127.0.0.1:8090/counter
curl http://127.0.0.1:8090/counter
curl -i -X POST http://127.0.0.1:8090/shutdown
```

The results are `0`, `5`, 409, `5`, and 204. Increments must be 1–1000 and the total at most 1000000. A non-integer JSON body gets 400; a full or stopped actor gets 503. Updates are not automatically resent. A timeout can follow a completed increment. This API has no request IDs or deduplication, and reading the total cannot identify whether a particular increment succeeded.

`/shutdown` stops the actor. HTTP remains running and returns 503 for counter requests afterward. Stop HTTP with Ctrl+C. Set `NAGI_SAMPLE_PORT` to change the port.

The Supervisor and HTTP server run in one scope. `monitor_task = spawn monitor(group)` creates `Task[Result[unit, Error]]`; `await monitor_task` yields `Result[Result[unit, Error], TaskFailure]`. After `await monitor_task`, the parent handles `Ok(inner)` with `try inner`, propagating a terminal Supervisor Err as the body Err. The scope requests HTTP cancellation and actually joins its direct children before returning the original Error. HTTP remains a legacy statement spawn, so its own Err also faults the scope. Normal `/shutdown` produces an inner Ok and leaves HTTP running. Printing a TaskFailure leaves the scope failed.

Replacing receipt with `discard(monitor_task)` would abandon the inner terminal Err and lose HTTP cancellation. HTTP state holds only Actor and Control; it does not retain the Supervisor context and create a cycle between shutdown and context release. Synchronous parent Future Drop only requests cancellation; it cannot confirm asynchronous termination or closure of arbitrary HTTP handlers. The runtime uses Tokio tasks within one process.

State lives in memory and resets after process shutdown or actor restart. Restarting does not provide persistence or rollback. This example listens locally and has no authentication for public deployment.

[日本語](README.md)
