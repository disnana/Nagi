package taskscope

import (
	"context"
	"errors"
	"runtime"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

func task[T any](work func() T) func(context.Context) T {
	return func(context.Context) T { return work() }
}

func signal(t *testing.T, channel <-chan struct{}) {
	t.Helper()
	select {
	case <-channel:
	case <-time.After(2 * time.Second):
		t.Fatal("barrier deadline exceeded")
	}
}

func closeGate(channel chan struct{}) {
	select {
	case <-channel:
	default:
		close(channel)
	}
}

func cleanupScope(t *testing.T, scope *Scope, gates ...chan struct{}) {
	t.Helper()
	t.Cleanup(func() {
		for _, gate := range gates {
			closeGate(gate)
		}
		done := make(chan struct{})
		go func() {
			_ = scope.Cancel()
			close(done)
		}()
		select {
		case <-done:
		case <-time.After(2 * time.Second):
			t.Error("scope fixture cleanup deadline exceeded")
		}
	})
}

func waitReceiptState[T any](t *testing.T, receipt *receipt[T], want receiptState) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for {
		receipt.mu.Lock()
		got := receipt.state
		receipt.mu.Unlock()
		if got == want {
			return
		}
		if time.Now().After(deadline) {
			t.Fatalf("receipt state = %d, want %d", got, want)
		}
		runtime.Gosched()
	}
}

func waitScopeClosed(t *testing.T, scope *Scope) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for {
		scope.mu.Lock()
		closed := scope.closed
		scope.mu.Unlock()
		if closed {
			return
		}
		if time.Now().After(deadline) {
			t.Fatal("scope was not sealed")
		}
		runtime.Gosched()
	}
}

func waitEntryReady(t *testing.T, scope *Scope, ticket uint64) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for {
		scope.mu.Lock()
		entry := scope.entries[ticket]
		ready := entry != nil && entry.ready
		scope.mu.Unlock()
		if ready {
			return
		}
		if time.Now().After(deadline) {
			t.Fatal("completion was not queued")
		}
		runtime.Gosched()
	}
}

func waitRequestedCancel(t *testing.T, scope *Scope, ticket uint64) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for {
		scope.mu.Lock()
		entry := scope.entries[ticket]
		requested := entry != nil && entry.requestedCancel && entry.cancel == nil
		scope.mu.Unlock()
		if requested {
			return
		}
		if time.Now().After(deadline) {
			t.Fatal("cancellation was not requested")
		}
		runtime.Gosched()
	}
}

func receive[T any](t *testing.T, scope *Scope, handle Handle[T]) T {
	t.Helper()
	value, problem := Receive(scope, handle)
	if problem != nil {
		t.Fatal(problem)
	}
	return value
}

func TestTaskScopeColdConcurrentChildren(t *testing.T) {
	type businessResult struct {
		ok      bool
		problem string
	}
	scope := New()
	started := make(chan int, 2)
	release := make(chan struct{})
	first, err := Spawn(scope, task(func() int { started <- 1; <-release; return 10 }), nil)
	if err != nil {
		t.Fatal(err)
	}
	second, err := Spawn(scope, task(func() int { started <- 2; <-release; return 20 }), nil)
	if err != nil {
		t.Fatal(err)
	}
	seen := map[int]bool{}
	seen[<-started], seen[<-started] = true, true
	if !seen[1] || !seen[2] {
		t.Fatalf("children did not start independently: %v", seen)
	}
	close(release)
	if got := receive(t, scope, first); got != 10 {
		t.Fatalf("first output = %d", got)
	}
	if got := receive(t, scope, second); got != 20 {
		t.Fatalf("second output = %d", got)
	}
	business, err := Spawn(scope, task(func() businessResult {
		return businessResult{problem: "rejected"}
	}), nil)
	if err != nil {
		t.Fatal(err)
	}
	if got := receive(t, scope, business); got.ok || got.problem != "rejected" {
		t.Fatalf("business output was flattened or changed: %+v", got)
	}
	if err := scope.Finish(); err != nil {
		t.Fatal(err)
	}
}

func TestTaskScopeReceiveWaitsForCleanupButNotSibling(t *testing.T) {
	scope := New()
	cleanupStarted := make(chan struct{})
	cleanupRelease := make(chan struct{})
	blocked, err := Spawn(scope, task(func() string { return "value" }), func() {
		close(cleanupStarted)
		<-cleanupRelease
	})
	if err != nil {
		t.Fatal(err)
	}
	siblingRelease := make(chan struct{})
	sibling, err := Spawn(scope, task(func() int { <-siblingRelease; return 2 }), nil)
	if err != nil {
		t.Fatal(err)
	}
	signal(t, cleanupStarted)
	received := make(chan string, 1)
	go func() {
		value, problem := Receive(scope, blocked)
		if problem != nil {
			received <- problem.Error()
			return
		}
		received <- value
	}()
	select {
	case value := <-received:
		t.Fatalf("receive completed before cleanup: %q", value)
	default:
	}
	close(cleanupRelease)
	select {
	case value := <-received:
		if value != "value" {
			t.Fatalf("output changed: %q", value)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("receive waited for unrelated sibling")
	}
	close(siblingRelease)
	if err := Discard(scope, sibling); err != nil {
		t.Fatal(err)
	}
	if err := scope.Finish(); err != nil {
		t.Fatal(err)
	}
}

func TestTaskScopeDiscardKeepsJoinAndReleasesOutput(t *testing.T) {
	type payload struct{ bytes []byte }
	for _, discardAfterReady := range []bool{false, true} {
		scope := New()
		started := make(chan struct{})
		release := make(chan struct{})
		cleaned := make(chan struct{})
		handle, err := Spawn(scope, task(func() payload {
			close(started)
			<-release
			return payload{bytes: make([]byte, 1024)}
		}), func() { close(cleaned) })
		if err != nil {
			t.Fatal(err)
		}
		signal(t, started)
		if discardAfterReady {
			close(release)
			signal(t, cleaned)
			signal(t, handle.receipt.done)
		}
		if err := Discard(scope, handle); err != nil {
			t.Fatal(err)
		}
		if !discardAfterReady {
			close(release)
		}
		if err := scope.Finish(); err != nil {
			t.Fatal(err)
		}
		if handle.receipt.value != nil || len(scope.entries) != 0 {
			t.Fatal("discard retained output or join record")
		}
	}
}

func TestTaskScopeHandleIdentityAndSingleConsumption(t *testing.T) {
	owner, other := New(), New()
	handle, err := Spawn(owner, task(func() int { return 7 }), nil)
	if err != nil {
		t.Fatal(err)
	}
	copyOfHandle := handle
	if _, err := Receive(other, handle); err != ErrWrongScope {
		t.Fatalf("wrong-scope receive = %v", err)
	}
	if err := Discard(other, handle); err != ErrWrongScope {
		t.Fatalf("wrong-scope discard = %v", err)
	}
	if got := receive(t, owner, copyOfHandle); got != 7 {
		t.Fatalf("output = %d", got)
	}
	if _, err := Receive(owner, handle); err != ErrInvalid && err != ErrConsumed {
		t.Fatalf("double receive = %v", err)
	}
	if err := Discard(owner, handle); err != ErrInvalid && err != ErrConsumed {
		t.Fatalf("receive then discard = %v", err)
	}
	if _, err := Receive(owner, Handle[int]{}); err != ErrInvalid {
		t.Fatalf("zero handle = %v", err)
	}
	forged := Handle[int]{owner: owner.identity, ticket: handle.ticket + 1, receipt: handle.receipt}
	if _, err := Receive(owner, forged); err != ErrInvalid {
		t.Fatalf("forged handle = %v", err)
	}
	if err := owner.Finish(); err != nil {
		t.Fatal(err)
	}
}

func TestTaskScopeFailureNeverPublishesAValue(t *testing.T) {
	for _, fixture := range []struct {
		name    string
		work    func() int
		cleanup func()
		want    error
		related error
	}{
		{"work panic", func() int { panic("private") }, nil, ErrTaskPanicked, nil},
		{"cleanup panic", func() int { return 9 }, func() { panic("private") }, ErrCleanupPanicked, nil},
		{"work goexit", func() int { runtime.Goexit(); return 9 }, nil, ErrTaskGoexit, nil},
		{"cleanup goexit", func() int { return 9 }, func() { runtime.Goexit() }, ErrCleanupGoexit, nil},
		{"work and cleanup panic", func() int { panic("work") }, func() { panic("cleanup") }, ErrTaskPanicked, ErrCleanupPanicked},
	} {
		t.Run(fixture.name, func(t *testing.T) {
			scope := New()
			var cleanupCalls atomic.Int64
			cleanup := func() {
				cleanupCalls.Add(1)
				if fixture.cleanup != nil {
					fixture.cleanup()
				}
			}
			handle, err := Spawn(scope, task(fixture.work), cleanup)
			if err != nil {
				t.Fatal(err)
			}
			value, err := Receive(scope, handle)
			if !errors.Is(err, fixture.want) || value != 0 || handle.receipt.value != nil {
				t.Fatalf("value=%d error=%v", value, err)
			}
			if cleanupCalls.Load() != 1 {
				t.Fatalf("cleanup calls = %d", cleanupCalls.Load())
			}
			if err := scope.Finish(); !errors.Is(err, fixture.want) {
				t.Fatalf("sticky finish = %v", err)
			}
			if fixture.related == nil {
				if len(scope.related) != 0 {
					t.Fatalf("unexpected related faults: %v", scope.related)
				}
			} else if len(scope.related) != 1 || !errors.Is(scope.related[0], fixture.related) {
				t.Fatalf("related faults = %v", scope.related)
			}
		})
	}
}

func TestTaskScopeFinishSealsRegistrationAndJoinsDiscarded(t *testing.T) {
	scope := New()
	release := make(chan struct{})
	var cleaned atomic.Bool
	handle, err := Spawn(scope, task(func() int { <-release; return 1 }), func() { cleaned.Store(true) })
	if err != nil {
		t.Fatal(err)
	}
	if err := Discard(scope, handle); err != nil {
		t.Fatal(err)
	}
	finished := make(chan error, 1)
	go func() { finished <- scope.Finish() }()
	select {
	case err := <-finished:
		t.Fatalf("finish returned before child cleanup: %v", err)
	default:
	}
	close(release)
	select {
	case err := <-finished:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("finish deadline exceeded")
	}
	if !cleaned.Load() {
		t.Fatal("finish returned before cleanup")
	}
	if err := scope.Finish(); err != nil {
		t.Fatalf("repeated finish = %v", err)
	}
	if _, err := Spawn(scope, task(func() int { return 2 }), nil); err != ErrClosed {
		t.Fatalf("spawn after finish = %v", err)
	}

	unconsumed := New()
	if _, err := Spawn(unconsumed, task(func() int { return 3 }), nil); err != nil {
		t.Fatal(err)
	}
	if err := unconsumed.Finish(); err != ErrUnconsumed {
		t.Fatalf("unconsumed finish = %v", err)
	}
}

func TestTaskScopeConcurrentConsumersShareReceiptState(t *testing.T) {
	scope := New()
	release := make(chan struct{})
	handle, err := Spawn(scope, task(func() int { <-release; return 4 }), nil)
	if err != nil {
		t.Fatal(err)
	}
	var successes atomic.Int64
	var consumed atomic.Int64
	var wait sync.WaitGroup
	for range 2 {
		wait.Add(1)
		go func(copy Handle[int]) {
			defer wait.Done()
			value, problem := Receive(scope, copy)
			switch {
			case problem == nil && value == 4:
				successes.Add(1)
			case problem == ErrConsumed:
				consumed.Add(1)
			case problem == ErrInvalid:
				// The winner can retire the record before the loser validates it.
				consumed.Add(1)
			default:
				t.Errorf("unexpected consumer result: value=%d error=%v", value, problem)
			}
		}(handle)
	}
	close(release)
	wait.Wait()
	if successes.Load() != 1 || consumed.Load() != 1 {
		t.Fatalf("success=%d consumed=%d", successes.Load(), consumed.Load())
	}
	if err := scope.Finish(); err != nil {
		t.Fatal(err)
	}
}

func TestTaskScopeFinishOverlapsClaimedReceiveAndCleanup(t *testing.T) {
	scope := New()
	cleanupStarted := make(chan struct{})
	cleanupRelease := make(chan struct{})
	cleanupScope(t, scope, cleanupRelease)
	handle, err := Spawn(scope, task(func() int { return 11 }), func() {
		close(cleanupStarted)
		<-cleanupRelease
	})
	if err != nil {
		t.Fatal(err)
	}
	signal(t, cleanupStarted)
	received := make(chan struct {
		value   int
		problem error
	}, 1)
	go func() {
		value, problem := Receive(scope, handle)
		received <- struct {
			value   int
			problem error
		}{value, problem}
	}()
	waitReceiptState(t, handle.receipt, receiptReceiving)
	if err := Discard(scope, handle); err != ErrConsumed {
		t.Fatalf("discard after receive claim = %v", err)
	}

	finished := make(chan error, 1)
	go func() { finished <- scope.Finish() }()
	waitScopeClosed(t, scope)
	if _, err := Spawn(scope, task(func() int { return 12 }), nil); err != ErrClosed {
		t.Fatalf("spawn after overlapping finish = %v", err)
	}
	select {
	case result := <-received:
		t.Fatalf("receive completed before cleanup: value=%d error=%v", result.value, result.problem)
	case problem := <-finished:
		t.Fatalf("finish completed before cleanup: %v", problem)
	default:
	}

	close(cleanupRelease)
	select {
	case result := <-received:
		if result.problem != nil || result.value != 11 {
			t.Fatalf("receive result: value=%d error=%v", result.value, result.problem)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("receive deadline exceeded")
	}
	select {
	case problem := <-finished:
		if problem != nil {
			t.Fatal(problem)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("finish deadline exceeded")
	}
}

func TestTaskScopeBusinessErrorAndCancelledValueStayValues(t *testing.T) {
	scope := New()
	siblingStarted := make(chan struct{})
	siblingRelease := make(chan struct{})
	cleanupScope(t, scope, siblingRelease)
	siblingCancelled := make(chan struct{})
	sibling, err := Spawn(scope, func(ctx context.Context) int {
		close(siblingStarted)
		select {
		case <-ctx.Done():
			close(siblingCancelled)
			return -1
		case <-siblingRelease:
			return 7
		}
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	signal(t, siblingStarted)
	business, err := Spawn(scope, func(context.Context) error { return context.Canceled }, nil)
	if err != nil {
		t.Fatal(err)
	}
	value, problem := Receive(scope, business)
	if problem != nil || value != context.Canceled {
		t.Fatalf("business value=%v problem=%v", value, problem)
	}
	select {
	case <-siblingCancelled:
		t.Fatal("business error value cancelled sibling")
	default:
	}
	close(siblingRelease)
	if got := receive(t, scope, sibling); got != 7 {
		t.Fatalf("sibling output = %d", got)
	}
	if err := scope.Finish(); err != nil {
		t.Fatal(err)
	}
}

func TestTaskScopeObservedFaultCancelsAndDrainsSibling(t *testing.T) {
	scope := New()
	siblingStarted := make(chan struct{})
	siblingCancelled := make(chan struct{})
	siblingCleanupStarted := make(chan struct{})
	siblingCleanupRelease := make(chan struct{})
	cleanupScope(t, scope, siblingCleanupRelease)
	sibling, err := Spawn(scope, func(ctx context.Context) int {
		close(siblingStarted)
		<-ctx.Done()
		close(siblingCancelled)
		return 4
	}, func() {
		close(siblingCleanupStarted)
		<-siblingCleanupRelease
	})
	if err != nil {
		t.Fatal(err)
	}
	signal(t, siblingStarted)
	var faultCleanup atomic.Int64
	fault, err := Spawn(scope, func(context.Context) int { panic("private") }, func() {
		faultCleanup.Add(1)
	})
	if err != nil {
		t.Fatal(err)
	}
	signal(t, fault.receipt.done)
	waitEntryReady(t, scope, fault.ticket)
	select {
	case <-siblingCancelled:
		t.Fatal("unobserved child fault interrupted scope body")
	default:
	}

	received := make(chan error, 1)
	go func() {
		_, problem := Receive(scope, fault)
		received <- problem
	}()
	signal(t, siblingCancelled)
	signal(t, siblingCleanupStarted)
	lateCancelled := make(chan struct{})
	late, err := Spawn(scope, func(ctx context.Context) int {
		<-ctx.Done()
		close(lateCancelled)
		return 8
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	signal(t, lateCancelled)
	if err := Discard(scope, late); err != nil {
		t.Fatal(err)
	}
	select {
	case problem := <-received:
		t.Fatalf("fault receive returned before sibling cleanup: %v", problem)
	default:
	}
	close(siblingCleanupRelease)
	select {
	case problem := <-received:
		if !errors.Is(problem, ErrTaskPanicked) {
			t.Fatalf("fault receive = %v", problem)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("fault receive deadline exceeded")
	}
	if faultCleanup.Load() != 1 {
		t.Fatalf("fault cleanup calls = %d", faultCleanup.Load())
	}
	if err := scope.Finish(); !errors.Is(err, ErrTaskPanicked) {
		t.Fatalf("sticky finish = %v", err)
	}
	if err := Discard(scope, sibling); err != nil {
		t.Fatal(err)
	}
}

func TestTaskScopeDiscardNeverLosesUnobservedFault(t *testing.T) {
	for _, afterCompletion := range []bool{false, true} {
		scope := New()
		release := make(chan struct{})
		cleanupScope(t, scope, release)
		handle, err := Spawn(scope, func(context.Context) int {
			<-release
			panic("private")
		}, nil)
		if err != nil {
			t.Fatal(err)
		}
		if afterCompletion {
			close(release)
			signal(t, handle.receipt.done)
			waitEntryReady(t, scope, handle.ticket)
		}
		if err := Discard(scope, handle); err != nil {
			t.Fatal(err)
		}
		if !afterCompletion {
			close(release)
		}
		if err := scope.Finish(); !errors.Is(err, ErrTaskPanicked) {
			t.Fatalf("discard afterCompletion=%v finish=%v", afterCompletion, err)
		}
		if len(scope.entries) != 0 {
			t.Fatal("recorded discarded fault retained an entry")
		}
		if err := scope.Finish(); !errors.Is(err, ErrTaskPanicked) {
			t.Fatalf("repeated sticky finish = %v", err)
		}
	}
}

func TestTaskScopeFinishObservesFaultBeforeBlockedSibling(t *testing.T) {
	scope := New()
	siblingStarted := make(chan struct{})
	siblingCancelled := make(chan struct{})
	siblingRelease := make(chan struct{})
	cleanupScope(t, scope, siblingRelease)
	sibling, err := Spawn(scope, func(ctx context.Context) int {
		close(siblingStarted)
		<-ctx.Done()
		close(siblingCancelled)
		<-siblingRelease
		return 1
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	signal(t, siblingStarted)
	fault, err := Spawn(scope, func(context.Context) int { panic("private") }, nil)
	if err != nil {
		t.Fatal(err)
	}
	signal(t, fault.receipt.done)
	waitEntryReady(t, scope, fault.ticket)
	if err := Discard(scope, sibling); err != nil {
		t.Fatal(err)
	}
	if err := Discard(scope, fault); err != nil {
		t.Fatal(err)
	}
	finished := make(chan error, 1)
	go func() { finished <- scope.Finish() }()
	signal(t, siblingCancelled)
	select {
	case problem := <-finished:
		t.Fatalf("finish returned before blocked sibling: %v", problem)
	default:
	}
	close(siblingRelease)
	select {
	case problem := <-finished:
		if !errors.Is(problem, ErrTaskPanicked) {
			t.Fatalf("finish = %v", problem)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("finish deadline exceeded")
	}
}

func TestTaskScopePublicationOrderChoosesPrimaryAndRelated(t *testing.T) {
	scope := New()
	firstRelease := make(chan struct{})
	secondRelease := make(chan struct{})
	cleanupScope(t, scope, firstRelease, secondRelease)
	first, err := Spawn(scope, func(context.Context) int { return 1 }, func() {
		<-firstRelease
		panic("cleanup")
	})
	if err != nil {
		t.Fatal(err)
	}
	second, err := Spawn(scope, func(context.Context) int {
		<-secondRelease
		panic("work")
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	close(firstRelease)
	signal(t, first.receipt.done)
	waitEntryReady(t, scope, first.ticket)
	close(secondRelease)
	signal(t, second.receipt.done)
	waitEntryReady(t, scope, second.ticket)
	if err := Discard(scope, first); err != nil {
		t.Fatal(err)
	}
	if err := Discard(scope, second); err != nil {
		t.Fatal(err)
	}
	if err := scope.Finish(); !errors.Is(err, ErrCleanupPanicked) {
		t.Fatalf("primary = %v", err)
	}
	if len(scope.related) != 1 || !errors.Is(scope.related[0], ErrTaskPanicked) {
		t.Fatalf("related = %v", scope.related)
	}
}

func TestTaskScopeBodyErrorPrecedesObservedFaultAndDrains(t *testing.T) {
	type bodyFailure struct{ message string }

	scope := New()
	siblingStarted := make(chan struct{})
	siblingCancelled := make(chan struct{})
	cleanupStarted := make(chan struct{})
	cleanupRelease := make(chan struct{})
	cleanupScope(t, scope, cleanupRelease)
	sibling, err := Spawn(scope, func(ctx context.Context) int {
		close(siblingStarted)
		<-ctx.Done()
		close(siblingCancelled)
		panic("late private fault")
	}, func() {
		close(cleanupStarted)
		<-cleanupRelease
	})
	if err != nil {
		t.Fatal(err)
	}
	signal(t, siblingStarted)
	fault, err := Spawn(scope, func(context.Context) int { panic("private") }, nil)
	if err != nil {
		t.Fatal(err)
	}
	signal(t, fault.receipt.done)
	waitEntryReady(t, scope, fault.ticket)
	if !scope.recordReady() {
		t.Fatal("completed fault was not recorded")
	}
	if !errors.Is(scope.primary, ErrTaskPanicked) {
		t.Fatalf("recorded primary = %v", scope.primary)
	}
	if err := Discard(scope, fault); err != nil {
		t.Fatal(err)
	}
	if err := Discard(scope, sibling); err != nil {
		t.Fatal(err)
	}
	signal(t, siblingCancelled)
	signal(t, cleanupStarted)

	body := &bodyFailure{message: "original body error"}
	exited := make(chan *bodyFailure, 1)
	go func() { exited <- FinishBodyError(scope, body) }()
	waitScopeClosed(t, scope)
	select {
	case problem := <-exited:
		t.Fatalf("body error returned before child cleanup: %p", problem)
	default:
	}
	close(cleanupRelease)
	select {
	case problem := <-exited:
		if problem != body {
			t.Fatalf("body error identity changed: got %p want %p", problem, body)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("body error drain deadline exceeded")
	}
	if !errors.Is(scope.primary, ErrTaskPanicked) {
		t.Fatalf("child diagnostic was lost: %v", scope.primary)
	}
	if len(scope.related) != 1 || !errors.Is(scope.related[0], ErrTaskPanicked) {
		t.Fatalf("drain fault was not retained as related: %v", scope.related)
	}
	if len(scope.entries) != 0 {
		t.Fatalf("body error retained %d child entries", len(scope.entries))
	}
}

func TestTaskScopeOwnerTeardownRequestsOnlyAndSeals(t *testing.T) {
	for iteration := 0; iteration < 64; iteration++ {
		scope := New()
		started := make(chan struct{})
		cancelled := make(chan struct{})
		cleanupStarted := make(chan struct{})
		cleanupRelease := make(chan struct{})
		cleanupScope(t, scope, cleanupRelease)
		handle, err := Spawn(scope, func(ctx context.Context) int {
			close(started)
			<-ctx.Done()
			close(cancelled)
			return iteration
		}, func() {
			close(cleanupStarted)
			<-cleanupRelease
		})
		if err != nil {
			t.Fatal(err)
		}
		signal(t, started)
		if err := Discard(scope, handle); err != nil {
			t.Fatal(err)
		}

		scope.RequestOwnerTeardown()
		scope.RequestOwnerTeardown()
		signal(t, cancelled)
		signal(t, cleanupStarted)
		select {
		case <-handle.receipt.done:
			t.Fatalf("iteration %d teardown reported cleanup complete", iteration)
		default:
		}
		if _, err := Spawn(scope, task(func() int { return 99 }), nil); err != ErrClosed {
			t.Fatalf("iteration %d spawn after teardown = %v", iteration, err)
		}
		waitRequestedCancel(t, scope, handle.ticket)
		close(cleanupRelease)
		if err := scope.Cancel(); err != nil {
			t.Fatalf("iteration %d drain = %v", iteration, err)
		}
		if len(scope.entries) != 0 {
			t.Fatalf("iteration %d retained %d entries", iteration, len(scope.entries))
		}
	}
}

func TestTaskScopeCancelDrainsWithoutInventingFailureOrConsumption(t *testing.T) {
	t.Run("normal business value after cancellation", func(t *testing.T) {
		scope := New()
		cleanupScope(t, scope)
		started := make(chan struct{})
		handle, err := Spawn(scope, func(ctx context.Context) error {
			close(started)
			<-ctx.Done()
			return context.Canceled
		}, nil)
		if err != nil {
			t.Fatal(err)
		}
		signal(t, started)
		if err := scope.Cancel(); err != nil {
			t.Fatalf("cancel = %v", err)
		}
		value, problem := Receive(scope, handle)
		if problem != nil || value != context.Canceled {
			t.Fatalf("value=%v problem=%v", value, problem)
		}
		if err := scope.Finish(); err != nil {
			t.Fatal(err)
		}
	})

	t.Run("noncooperative callback", func(t *testing.T) {
		scope := New()
		started := make(chan struct{})
		release := make(chan struct{})
		cleanupScope(t, scope, release)
		handle, err := Spawn(scope, func(context.Context) int {
			close(started)
			<-release
			return 2
		}, nil)
		if err != nil {
			t.Fatal(err)
		}
		signal(t, started)
		cancelled := make(chan error, 1)
		go func() { cancelled <- scope.Cancel() }()
		waitRequestedCancel(t, scope, handle.ticket)
		select {
		case problem := <-cancelled:
			t.Fatalf("cancel returned before callback: %v", problem)
		default:
		}
		close(release)
		select {
		case problem := <-cancelled:
			if problem != nil {
				t.Fatal(problem)
			}
		case <-time.After(2 * time.Second):
			t.Fatal("cancel deadline exceeded")
		}
		if err := Discard(scope, handle); err != nil {
			t.Fatal(err)
		}
		if err := scope.Finish(); err != nil {
			t.Fatal(err)
		}
	})

	t.Run("panic after cancellation remains a fault", func(t *testing.T) {
		scope := New()
		started := make(chan struct{})
		panicRelease := make(chan struct{})
		cleanupScope(t, scope, panicRelease)
		var cleaned atomic.Bool
		handle, err := Spawn(scope, func(ctx context.Context) int {
			close(started)
			<-ctx.Done()
			<-panicRelease
			panic("after cancellation")
		}, func() { cleaned.Store(true) })
		if err != nil {
			t.Fatal(err)
		}
		signal(t, started)
		cancelled := make(chan error, 1)
		go func() { cancelled <- scope.Cancel() }()
		waitRequestedCancel(t, scope, handle.ticket)
		close(panicRelease)
		select {
		case problem := <-cancelled:
			if !errors.Is(problem, ErrTaskPanicked) {
				t.Fatalf("cancel = %v", problem)
			}
		case <-time.After(2 * time.Second):
			t.Fatal("cancel deadline exceeded")
		}
		if !cleaned.Load() {
			t.Fatal("fault cleanup did not run")
		}
		if err := Discard(scope, handle); err != nil {
			t.Fatal(err)
		}
		if err := scope.Finish(); !errors.Is(err, ErrTaskPanicked) {
			t.Fatalf("sticky finish = %v", err)
		}
	})
}

func TestTaskScopeConcurrentReceiveAndDiscardHaveOneWinner(t *testing.T) {
	for iteration := 0; iteration < 64; iteration++ {
		scope := New()
		start := make(chan struct{})
		release := make(chan struct{})
		cleanupScope(t, scope, start, release)
		handle, err := Spawn(scope, func(context.Context) int {
			<-release
			return 5
		}, nil)
		if err != nil {
			t.Fatal(err)
		}
		received := make(chan struct {
			value   int
			problem error
		}, 1)
		discarded := make(chan error, 1)
		go func(copy Handle[int]) {
			<-start
			value, problem := Receive(scope, copy)
			received <- struct {
				value   int
				problem error
			}{value, problem}
		}(handle)
		go func(copy Handle[int]) {
			<-start
			discarded <- Discard(scope, copy)
		}(handle)
		close(start)
		close(release)
		receiveResult := <-received
		discardResult := <-discarded
		receiveWon := receiveResult.problem == nil && receiveResult.value == 5
		discardWon := discardResult == nil
		if receiveWon == discardWon {
			t.Fatalf("iteration %d receive=(%d,%v) discard=%v", iteration, receiveResult.value, receiveResult.problem, discardResult)
		}
		if !receiveWon && receiveResult.problem != ErrConsumed && receiveResult.problem != ErrInvalid {
			t.Fatalf("iteration %d receive loss = %v", iteration, receiveResult.problem)
		}
		if !discardWon && discardResult != ErrConsumed && discardResult != ErrInvalid {
			t.Fatalf("iteration %d discard loss = %v", iteration, discardResult)
		}
		if err := scope.Finish(); err != nil {
			t.Fatal(err)
		}
	}
}

func TestTaskScopeConcurrentCancelAndFinishShareDrain(t *testing.T) {
	scope := New()
	started := make(chan struct{})
	cleanupStarted := make(chan struct{})
	cleanupRelease := make(chan struct{})
	start := make(chan struct{})
	cleanupScope(t, scope, cleanupRelease, start)
	var cleanupCalls atomic.Int64
	handle, err := Spawn(scope, func(ctx context.Context) int {
		close(started)
		<-ctx.Done()
		return 6
	}, func() {
		cleanupCalls.Add(1)
		close(cleanupStarted)
		<-cleanupRelease
	})
	if err != nil {
		t.Fatal(err)
	}
	signal(t, started)
	if err := Discard(scope, handle); err != nil {
		t.Fatal(err)
	}
	cancelled := make(chan error, 1)
	finished := make(chan error, 1)
	go func() { <-start; cancelled <- scope.Cancel() }()
	go func() { <-start; finished <- scope.Finish() }()
	close(start)
	waitRequestedCancel(t, scope, handle.ticket)
	signal(t, cleanupStarted)
	select {
	case problem := <-cancelled:
		t.Fatalf("cancel returned before cleanup: %v", problem)
	case problem := <-finished:
		t.Fatalf("finish returned before cleanup: %v", problem)
	default:
	}
	close(cleanupRelease)
	for name, result := range map[string]<-chan error{"cancel": cancelled, "finish": finished} {
		select {
		case problem := <-result:
			if problem != nil {
				t.Fatalf("%s = %v", name, problem)
			}
		case <-time.After(2 * time.Second):
			t.Fatalf("%s deadline exceeded", name)
		}
	}
	if cleanupCalls.Load() != 1 || len(scope.entries) != 0 {
		t.Fatalf("cleanup=%d entries=%d", cleanupCalls.Load(), len(scope.entries))
	}
}
