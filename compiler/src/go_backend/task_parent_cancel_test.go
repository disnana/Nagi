package main

import (
	"context"
	"sync/atomic"
	"testing"
	"time"

	"nagi.internal/go-runtime/internal/taskscope"
)

func waitParentEvent[T any](t *testing.T, name string, event <-chan T) T {
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
	type received = nagiResult[nagiResult[int64, int64], nagiTaskFailure]
	observedReceive := make(chan received, 1)
	nagiTaskObserve = func(result received) { observedReceive <- result }
	if fixtureMode == "parent_wait_normal" {
		result, completed := nagiTaskParent(context.Background())
		if !completed || !result.ok {
			t.Fatalf("uncancelled parent: completed=%t result=%+v", completed, result)
		}
		got := waitParentEvent(t, "receive after parent timer", observedReceive)
		if !got.ok || !got.value.ok || got.value.value != 17 {
			t.Fatalf("receive = %+v", got)
		}
		return
	}

	parent, cancelParent := context.WithCancel(context.Background())
	defer cancelParent()
	childWaiting := make(chan struct{})
	childInterrupted := make(chan struct{})
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
	nagiTaskSleepStarted = func() { close(childWaiting) }
	nagiTaskSleepInterrupted = func() { close(childInterrupted) }
	nagiTaskCleanup = func() {
		close(cleanupEntered)
		<-releaseCleanup
		close(cleaned)
	}

	type observation struct {
		scope *taskscope.Scope
		state func() (ready, recorded, retained bool)
	}
	parentWaiting := make(chan observation, 1)
	cancelled := make(chan bool, 1)
	var cancellations atomic.Int32
	nagiTaskParentWaiting = func(scope *taskscope.Scope, handle taskscope.Handle[nagiResult[int64, int64]]) {
		state := taskscope.ObserveTaskExit(scope, handle, func(abandoned bool) {
			if cancellations.Add(1) == 1 {
				cancelled <- abandoned
			}
		})
		parentWaiting <- observation{scope, state}
	}
	caller := make(chan bool, 1)
	go func() {
		_, completed := nagiTaskParent(parent)
		caller <- completed
	}()
	observed := waitParentEvent(t, "parent timer creation", parentWaiting)
	waitParentEvent(t, "child timer creation", childWaiting)
	publication := taskscope.TaskPublication(observed.scope)
	cancelParent()
	if !waitParentEvent(t, "cancellation request", cancelled) {
		t.Fatal("child cancellation preceded output abandonment")
	}
	waitParentEvent(t, "child observing cancellation", childInterrupted)
	waitParentEvent(t, "child cleanup entry", cleanupEntered)
	if waitParentEvent(t, "parent exit while cleanup is blocked", caller) {
		t.Fatal("cancelled parent produced a result")
	}
	select {
	case got := <-observedReceive:
		t.Fatalf("parent received after cancellation: %+v", got)
	default:
	}
	if ready, recorded, retained := observed.state(); ready || recorded || !retained {
		t.Fatalf("scope while cleanup is blocked: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}

	close(releaseCleanup)
	waitParentEvent(t, "cleanup completion", cleaned)
	waitParentEvent(t, "scope completion publication", publication)
	if ready, recorded, retained := observed.state(); !ready || recorded || !retained {
		t.Fatalf("scope before independent finish: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}
	finished := make(chan error, 1)
	go func() { finished <- observed.scope.Finish() }()
	if err := waitParentEvent(t, "independent finish", finished); err != nil {
		t.Fatalf("finish after cancellation: %v", err)
	}
	if ready, recorded, retained := observed.state(); ready || !recorded || retained {
		t.Fatalf("scope after finish: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}
	if got := cancellations.Load(); got != 1 {
		t.Fatalf("child cancellation invoked %d times, want one", got)
	}
}
