# Private Go lifecycle and Task-scope kernels

This std-only module is an internal experiment after the Rust/Go conformance
slice. It is not copied into generated applications and exposes no Nagi API.
`std.actor`, public scope/task, mailbox, READY/events, and payload charging remain
unsupported by the Go backend.

`internal/taskscope` covers only the normal-success Task boundary. It starts
real goroutines after registering typed receipts, publishes an output only after
the callback and explicit cleanup return, consumes a copied handle at most once,
and keeps discarded outputs in the scope's join set until `Finish`. Go has no
physical goroutine join primitive: the closed completion channel proves the
callback, its defers, and cleanup returned, not that the runtime destroyed the
goroutine. Panic/fault propagation, cancellation, sticky primary faults, body
error precedence, and synchronous owner teardown are intentionally not connected.
Public Nagi Task/scope syntax remains rejected before the Go toolchain.

`internal/lifecycle` runs one manager and at most one active goroutine per child.
Registration is cold. Each generation invokes `Factory(context.Context) error`
inside a retained boundary. Worker errors and recoverable panics follow the
temporary/transient/permanent restart policy; business replies stay outside the
kernel. Restart attempts share a bounded sliding-window budget. Successful wait
means that child callbacks, their defers, and the explicit cleanup hook returned.

An incomplete wait retains the live obligations for a later waiter. The cleanup
hook runs outside locks after every child joins. No finalizer, GC cycle,
goroutine count, or cancel call is used as cleanup evidence. Native error text is
copied and bounded; arbitrary panic payloads are never formatted. OOM, process
exit/abort, fatal runtime faults, independently spawned goroutines, and blocking
native code are not made recoverable. A noncooperative callback can prevent
completion; the kernel reports incomplete rather than pretending it was killed.

Run with the selected real Go toolchain (not GNU Go):

```sh
GOTOOLCHAIN=local GOWORK=off CGO_ENABLED=0 go test -p=1 -count=1 -timeout=30s ./...
GOTOOLCHAIN=local GOWORK=off CGO_ENABLED=1 go test -race -p=1 -count=1 -timeout=30s ./...
GOTOOLCHAIN=local GOWORK=off CGO_ENABLED=0 go test -run '^$' -bench '^BenchmarkLifecycle$' -benchtime=1x -timeout=30s ./...
```

The race run needs a supported native C compiler. The finite benchmark covers
64/1024 idle child starts and complete shutdown, not actor mailbox throughput,
HTTP, or cross-language performance. See [the internal contract](../../docs/internal/go-lifecycle-kernel.md).
