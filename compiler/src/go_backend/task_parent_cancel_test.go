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
	switch fixtureMode {
	case "parent_receive_pending", "parent_receive_ready_cancel", "parent_receive_value":
		testParentReceive(t)
		return
	}
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

func testParentReceive(t *testing.T) {
	type value = nagiResult[int64, int64]
	type receipt = nagiResult[value, nagiTaskFailure]
	type observation struct {
		scope  *taskscope.Scope
		handle taskscope.Handle[value]
		state  func() (ready, recorded, retained bool)
	}

	parent, cancel := context.WithCancel(context.Background())
	defer cancel()
	childWaiting := make(chan struct{})
	childInterrupted := make(chan struct{})
	cleanupEntered := make(chan struct{})
	releaseCleanup := make(chan struct{})
	allowReceive := make(chan struct{})
	defer func() {
		for _, gate := range []chan struct{}{releaseCleanup, allowReceive} {
			select {
			case <-gate:
			default:
				close(gate)
			}
		}
	}()
	nagiTaskSleepStarted = func() { close(childWaiting) }
	nagiTaskSleepInterrupted = func() { close(childInterrupted) }
	nagiTaskCleanup = func() { close(cleanupEntered); <-releaseCleanup }

	claimed := make(chan observation, 1)
	cancelled := make(chan bool, 1)
	received := make(chan receipt, 1)
	nagiTaskObserve = func(got receipt) { received <- got }
	nagiTaskReceiveClaimed = func(scope *taskscope.Scope, handle taskscope.Handle[value]) {
		state := taskscope.ObserveTaskExit(scope, handle, func(abandoned bool) { cancelled <- abandoned })
		claimed <- observation{scope, handle, state}
		<-allowReceive
	}
	caller := make(chan bool, 1)
	go func() {
		_, completed := nagiTaskParent(parent)
		caller <- completed
	}()
	seen := waitParentEvent(t, "live receive claim", claimed)
	waitParentEvent(t, "child timer creation", childWaiting)
	if err := taskscope.Discard(seen.scope, seen.handle); err != taskscope.ErrConsumed {
		t.Fatalf("duplicate use during claim = %v", err)
	}
	publication := taskscope.TaskPublication(seen.scope)
	pending := fixtureMode == "parent_receive_pending"
	winningValue := fixtureMode == "parent_receive_value"
	if pending {
		close(allowReceive)
		cancel()
		if !waitParentEvent(t, "owner cancellation", cancelled) {
			t.Fatal("owner cancellation preceded receive abandonment")
		}
		waitParentEvent(t, "child observing cancellation", childInterrupted)
		waitParentEvent(t, "blocked child cleanup", cleanupEntered)
	} else {
		waitParentEvent(t, "child cleanup", cleanupEntered)
		close(releaseCleanup)
		waitParentEvent(t, "child publication while claim is held", publication)
		if !winningValue {
			cancel()
		}
		close(allowReceive)
	}

	if completed := waitParentEvent(t, "parent exit", caller); completed != winningValue {
		t.Fatalf("parent completed=%t, want %t", completed, winningValue)
	}
	if winningValue {
		cancel()
		if got := waitParentEvent(t, "committed receive", received); !got.ok || !got.value.ok || got.value.value != 17 {
			t.Fatalf("committed receive = %+v", got)
		}
	}
	select {
	case got := <-received:
		t.Fatalf("unexpected additional delivery: %+v", got)
	default:
	}
	if pending {
		if ready, recorded, retained := seen.state(); ready || recorded || !retained {
			t.Fatalf("blocked cleanup: ready=%t recorded=%t retained=%t", ready, recorded, retained)
		}
		close(releaseCleanup)
		waitParentEvent(t, "publication after cleanup release", publication)
	} else if !winningValue && !waitParentEvent(t, "release of queued output", cancelled) {
		t.Fatal("cancelled receive retained the queued value")
	}
	finished := make(chan error, 1)
	go func() { finished <- seen.scope.Finish() }()
	if err := waitParentEvent(t, "independent finish", finished); err != nil {
		t.Fatalf("finish after parent exit: %v", err)
	}
	if ready, recorded, retained := seen.state(); ready || !recorded || retained {
		t.Fatalf("after finish: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}
	if _, err := taskscope.Receive(seen.scope, seen.handle); err != taskscope.ErrInvalid {
		t.Fatalf("receive after retirement = %v", err)
	}
}
