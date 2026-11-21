package main

import (
	"nagi.internal/go-runtime/internal/taskscope"
	"sync/atomic"
	"testing"
	"time"
)

func expectBodyPanicEvent[T any](t *testing.T, name string, event <-chan T) T {
	t.Helper()
	select {
	case value := <-event:
		return value
	case <-time.After(5 * time.Second):
		t.Fatalf("timed out waiting for %s", name)
		var zero T
		return zero
	}
}

func TestCheckedTaskLowering(t *testing.T) {
	waiting := make(chan struct{})
	interrupted := make(chan struct{})
	cleanupEntered := make(chan struct{})
	releaseCleanup := make(chan struct{})
	cleaned := make(chan struct{})
	defer func() {
		select {
		case <-releaseCleanup:
		default:
			close(releaseCleanup)
		}
	}()
	if fixtureMode == "sleep_body_panic" {
		// Notifications only: the generated timer selects on the real ctx.
		nagiTaskSleepStarted = func() { close(waiting) }
		nagiTaskSleepInterrupted = func() { close(interrupted) }
	} else {
		unblockWait := make(chan struct{})
		defer close(unblockWait)
		nagiTaskChildDone = func(done <-chan struct{}) {
			close(waiting)
			select {
			case <-done:
				close(interrupted)
			case <-unblockWait:
			}
		}
	}
	nagiTaskCleanup = func() {
		close(cleanupEntered)
		<-releaseCleanup
		close(cleaned)
	}

	type observation struct {
		scope *taskscope.Scope
		state func() (ready, recorded, retained bool)
	}
	probe := make(chan observation, 1)
	cancelled := make(chan bool, 1)
	var cancellations atomic.Int32
	nagiTaskBeforeBodyAssert = func(scope *taskscope.Scope, handle taskscope.Handle[nagiResult[int64, int64]]) {
		state := taskscope.ObserveTaskExit(scope, handle, func(abandoned bool) {
			if cancellations.Add(1) == 1 {
				cancelled <- abandoned
			}
		})
		<-waiting
		probe <- observation{scope, state}
	}

	type outcome struct {
		recovered any
		returned  bool
	}
	caller := make(chan outcome, 1)
	go func() {
		returned := false
		defer func() { caller <- outcome{recover(), returned} }()
		nagiMain().await()
		returned = true
	}()
	observed := expectBodyPanicEvent(t, "scope before assertion", probe)
	if !expectBodyPanicEvent(t, "cancellation request", cancelled) {
		t.Fatal("cancellation was invoked before output abandonment")
	}
	expectBodyPanicEvent(t, "child observing cancellation", interrupted)
	expectBodyPanicEvent(t, "child cleanup", cleanupEntered)
	result := expectBodyPanicEvent(t, "caller unwind while cleanup is blocked", caller)
	if message, ok := result.recovered.(string); result.returned || !ok || message != "assertion failed" {
		t.Fatalf("caller outcome = %+v, want the assertion panic", result)
	}
	if ready, recorded, retained := observed.state(); ready || recorded || !retained {
		t.Fatalf("scope before cleanup: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}

	close(releaseCleanup)
	expectBodyPanicEvent(t, "child cleanup completion", cleaned)
	finished := make(chan error, 1)
	go func() { finished <- observed.scope.Finish() }()
	if err := expectBodyPanicEvent(t, "independent completion recording", finished); err != nil {
		t.Fatalf("scope recorded the body panic as a child fault or retained output: %v", err)
	}
	if ready, recorded, retained := observed.state(); ready || !recorded || retained {
		t.Fatalf("scope after finish: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}
	if got := cancellations.Load(); got != 1 {
		t.Fatalf("child cancellation invoked %d times, want 1", got)
	}
}
