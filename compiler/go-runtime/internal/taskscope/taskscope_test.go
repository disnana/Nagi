package taskscope

import (
	"errors"
	"runtime"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

func signal(t *testing.T, channel <-chan struct{}) {
	t.Helper()
	select {
	case <-channel:
	case <-time.After(2 * time.Second):
		t.Fatal("barrier deadline exceeded")
	}
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
	first, err := Spawn(scope, func() int { started <- 1; <-release; return 10 }, nil)
	if err != nil {
		t.Fatal(err)
	}
	second, err := Spawn(scope, func() int { started <- 2; <-release; return 20 }, nil)
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
	business, err := Spawn(scope, func() businessResult {
		return businessResult{problem: "rejected"}
	}, nil)
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
	blocked, err := Spawn(scope, func() string { return "value" }, func() {
		close(cleanupStarted)
		<-cleanupRelease
	})
	if err != nil {
		t.Fatal(err)
	}
	siblingRelease := make(chan struct{})
	sibling, err := Spawn(scope, func() int { <-siblingRelease; return 2 }, nil)
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
		handle, err := Spawn(scope, func() payload {
			close(started)
			<-release
			return payload{bytes: make([]byte, 1024)}
		}, func() { close(cleaned) })
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
	handle, err := Spawn(owner, func() int { return 7 }, nil)
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
	}{
		{"work panic", func() int { panic("private") }, nil, ErrTaskFailed},
		{"cleanup panic", func() int { return 9 }, func() { panic("private") }, ErrCleanupFailed},
		{"goexit", func() int { runtime.Goexit(); return 9 }, nil, ErrTaskFailed},
	} {
		t.Run(fixture.name, func(t *testing.T) {
			scope := New()
			handle, err := Spawn(scope, fixture.work, fixture.cleanup)
			if err != nil {
				t.Fatal(err)
			}
			value, err := Receive(scope, handle)
			if !errors.Is(err, fixture.want) || value != 0 || handle.receipt.value != nil {
				t.Fatalf("value=%d error=%v", value, err)
			}
			if err := scope.Finish(); err != nil {
				t.Fatal(err)
			}
		})
	}
}

func TestTaskScopeFinishSealsRegistrationAndJoinsDiscarded(t *testing.T) {
	scope := New()
	release := make(chan struct{})
	var cleaned atomic.Bool
	handle, err := Spawn(scope, func() int { <-release; return 1 }, func() { cleaned.Store(true) })
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
	if _, err := Spawn(scope, func() int { return 2 }, nil); err != ErrClosed {
		t.Fatalf("spawn after finish = %v", err)
	}

	unconsumed := New()
	if _, err := Spawn(unconsumed, func() int { return 3 }, nil); err != nil {
		t.Fatal(err)
	}
	if err := unconsumed.Finish(); err != ErrUnconsumed {
		t.Fatalf("unconsumed finish = %v", err)
	}
}

func TestTaskScopeConcurrentConsumersShareReceiptState(t *testing.T) {
	scope := New()
	release := make(chan struct{})
	handle, err := Spawn(scope, func() int { <-release; return 4 }, nil)
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
	handle, err := Spawn(scope, func() int { return 11 }, func() {
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
	if _, err := Spawn(scope, func() int { return 12 }, nil); err != ErrClosed {
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
