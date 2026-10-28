# Private Go lifecycle kernel

This std-only module is an internal experiment after the Rust/Go conformance
slice. It is not copied into generated applications and exposes no Nagi API.
`std.actor`, async, scope/task, mailbox, READY/events, and payload charging remain
unsupported by the Go backend.

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
