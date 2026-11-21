package main

import (
	"context"
	"testing"
	"time"

	"nagi.internal/go-runtime/internal/taskscope"
)

func waitSiblingEvent[T any](t *testing.T, limit context.Context, name string, event <-chan T) T {
	t.Helper()
	select {
	case got := <-event:
		return got
	case <-limit.Done():
		t.Fatalf("timed out waiting for %s", name)
		var zero T
		return zero
	}
}

func TestCheckedTaskLowering(t *testing.T) {
	type result = nagiResult[nagiUnit, nagiError]
	type receipt = nagiResult[nagiResult[int64, int64], nagiTaskFailure]
	limit, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	timerStarted := make(chan struct{})
	interrupted := make(chan struct{})
	cleanupEntered := make(chan struct{})
	allowPanic := make(chan struct{})
	releaseCleanup := make(chan struct{})
	defer func() {
		for _, gate := range []chan struct{}{allowPanic, releaseCleanup} {
			select {
			case <-gate:
			default:
				close(gate)
			}
		}
	}()

	scopeSeen := make(chan *taskscope.Scope, 1)
	received := make(chan receipt, 1)
	returned := make(chan result, 1)
	nagiTaskSiblingScope = func(scope *taskscope.Scope) { scopeSeen <- scope }
	nagiTaskChildDone = func(<-chan struct{}) { <-allowPanic }
	nagiTaskSleepStarted = func() { close(timerStarted) }
	nagiTaskSleepInterrupted = func() { close(interrupted) }
	nagiTaskCleanup = func() {
		close(cleanupEntered)
		<-releaseCleanup
		panic("secondary cleanup failure")
	}
	nagiTaskObserve = func(value receipt) { received <- value }

	go func() { returned <- nagiMain().await() }()
	scope := waitSiblingEvent(t, limit, "both registrations and sibling discard", scopeSeen)
	waitSiblingEvent(t, limit, "generated sibling timer start", timerStarted)
	if entries, queued := taskscope.TaskOutstanding(scope); entries != 2 || queued != 0 {
		t.Fatalf("before primary panic: entries=%d queued=%d", entries, queued)
	}
	select {
	case <-interrupted:
		t.Fatal("sibling interrupted before primary panic")
	default:
	}

	// The gate allows the generated assertion to run. Only the real Receive
	// can record its fault and request cancellation of the generated timer.
	close(allowPanic)
	waitSiblingEvent(t, limit, "sibling taking its generated cancellation branch", interrupted)
	waitSiblingEvent(t, limit, "sibling entering cleanup", cleanupEntered)
	if primary, related := taskscope.TaskFaults(scope); primary != taskscope.ErrTaskPanicked || len(related) != 0 {
		t.Fatalf("during cleanup: primary=%v related=%v", primary, related)
	}
	if entries, queued := taskscope.TaskOutstanding(scope); entries != 2 || queued != 0 {
		t.Fatalf("during cleanup: entries=%d queued=%d", entries, queued)
	}
	select {
	case got := <-received:
		t.Fatalf("receive returned before sibling cleanup: %+v", got)
	case got := <-returned:
		t.Fatalf("scope returned before sibling cleanup: %+v", got)
	default:
	}

	close(releaseCleanup)
	got := waitSiblingEvent(t, limit, "typed receive", received)
	if got.ok || got.problem.kind != nagiTaskPanicked || got.problem.message != "child panicked" {
		t.Fatalf("receive = %+v", got)
	}
	exit := waitSiblingEvent(t, limit, "scope result", returned)
	if exit.ok || exit.problem.kind != nagiTaskInternal || exit.problem.message != got.problem.message {
		t.Fatalf("scope exit = %+v, receive = %+v", exit, got)
	}
	if primary, related := taskscope.TaskFaults(scope); primary != taskscope.ErrTaskPanicked ||
		len(related) != 1 || related[0] != taskscope.ErrCleanupPanicked {
		t.Fatalf("after cleanup: primary=%v related=%v", primary, related)
	}
	if entries, queued := taskscope.TaskOutstanding(scope); entries != 0 || queued != 0 {
		t.Fatalf("after finish: entries=%d queued=%d", entries, queued)
	}
}
