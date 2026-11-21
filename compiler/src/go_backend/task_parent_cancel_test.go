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
	case "parent_permit_cancel", "parent_permit_normal", "parent_permit_cleanup_fault":
		testParentPermit(t)
		return
	case "parent_finish_pending", "parent_finish_ready_cancel", "parent_finish_normal":
		testParentFinish(t)
		return
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

func testParentPermit(t *testing.T) {
	type value = nagiResult[int64, int64]
	type observation struct {
		scope *taskscope.Scope
		state func() (ready, recorded, retained bool)
	}
	parent, cancel := context.WithCancel(context.Background())
	defer cancel()
	permit := make(chan struct{}, 1)
	permit <- struct{}{}
	acquired, interrupted := make(chan struct{}), make(chan struct{})
	cleanupEntered, releaseCleanup, released := make(chan struct{}), make(chan struct{}), make(chan struct{})
	releaseFinish := make(chan struct{})
	defer func() {
		for _, gate := range []chan struct{}{releaseCleanup, releaseFinish} {
			select {
			case <-gate:
			default:
				close(gate)
			}
		}
	}()
	wantsCancel := fixtureMode == "parent_permit_cancel"
	wantsFault := fixtureMode == "parent_permit_cleanup_fault"
	var releases atomic.Int32
	// The test injects a permit at the generated timer's start and returns it
	// through the kernel's existing cleanup boundary, including on unwind.
	nagiTaskSleepStarted = func() { <-permit; close(acquired) }
	nagiTaskSleepInterrupted = func() { close(interrupted) }
	nagiTaskCleanup = func() {
		defer func() {
			releases.Add(1)
			select {
			case permit <- struct{}{}:
			default:
				panic("capacity-one test permit released twice")
			}
			close(released)
		}()
		close(cleanupEntered)
		<-releaseCleanup
		if wantsFault {
			panic("controlled cleanup fault")
		}
	}

	waiting := make(chan observation, 1)
	nagiTaskFinishWaiting = func(scope *taskscope.Scope, handle taskscope.Handle[value]) {
		waiting <- observation{scope, taskscope.ObserveTaskExit(scope, handle, func(bool) {})}
		<-releaseFinish
	}
	type outcome struct {
		returned, completed, ok bool
		fault                   any
	}
	caller := make(chan outcome, 1)
	go func() {
		var result outcome
		defer func() { result.fault = recover(); caller <- result }()
		value, completed := nagiTaskParent(parent)
		result = outcome{returned: true, completed: completed, ok: value.ok}
	}()
	waitParentEvent(t, "permit acquisition", acquired)
	seen := waitParentEvent(t, "first finish wait", waiting)
	publication := taskscope.TaskPublication(seen.scope)

	if wantsCancel {
		close(releaseFinish)
		cancel()
		waitParentEvent(t, "timer observing cancellation", interrupted)
	}
	waitParentEvent(t, "cleanup entry", cleanupEntered)
	if wantsCancel {
		if got := waitParentEvent(t, "parent exit before permit release", caller); !got.returned || got.completed || got.fault != nil {
			t.Fatalf("cancelled parent outcome = %+v", got)
		}
	}
	select {
	case <-permit:
		t.Fatal("permit was available during blocked cleanup")
	default:
	}
	if ready, recorded, retained := seen.state(); ready || recorded || !retained {
		t.Fatalf("blocked cleanup: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}
	close(releaseCleanup)
	waitParentEvent(t, "permit returned", released)
	waitParentEvent(t, "scope publication", publication)
	waitParentEvent(t, "readmission with returned permit", permit)
	select {
	case <-permit:
		t.Fatal("second acquisition succeeded while the sole permit was held")
	default:
	}
	if got := releases.Load(); got != 1 {
		t.Fatalf("cleanup returned %d permits", got)
	}
	if ready, recorded, retained := seen.state(); !ready || recorded || !retained {
		t.Fatalf("before finish recording: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}
	if !wantsCancel {
		close(releaseFinish)
		got := waitParentEvent(t, "uncancelled parent", caller)
		if wantsFault {
			if got.returned || got.fault != taskscope.ErrCleanupPanicked {
				t.Fatalf("cleanup fault outcome = %+v", got)
			}
		} else if !got.returned || !got.ok || !got.completed || got.fault != nil {
			t.Fatalf("normal outcome = %+v", got)
		}
	}
	finished := make(chan error, 1)
	go func() { finished <- seen.scope.Finish() }()
	fault := waitParentEvent(t, "independent finish", finished)
	if (wantsFault && fault != taskscope.ErrCleanupPanicked) || (!wantsFault && fault != nil) {
		t.Fatalf("finish fault = %v", fault)
	}
	if ready, recorded, retained := seen.state(); ready || !recorded || retained {
		t.Fatalf("after finish: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}
	permit <- struct{}{}
}

func testParentFinish(t *testing.T) {
	type value = nagiResult[int64, int64]
	type observation struct {
		scope *taskscope.Scope
		state func() (ready, recorded, retained bool)
	}
	parent, cancel := context.WithCancel(context.Background())
	defer cancel()
	timerStarted, interrupted := make(chan struct{}), make(chan struct{})
	cleanupEntered, releaseCleanup := make(chan struct{}), make(chan struct{})
	releaseWait := make(chan struct{})
	defer func() {
		for _, gate := range []chan struct{}{releaseCleanup, releaseWait} {
			select {
			case <-gate:
			default:
				close(gate)
			}
		}
	}()
	nagiTaskSleepStarted = func() { close(timerStarted) }
	nagiTaskSleepInterrupted = func() { close(interrupted) }
	nagiTaskCleanup = func() { close(cleanupEntered); <-releaseCleanup }
	waiting := make(chan observation, 1)
	cancelled := make(chan bool, 1)
	nagiTaskFinishWaiting = func(scope *taskscope.Scope, handle taskscope.Handle[value]) {
		state := taskscope.ObserveTaskExit(scope, handle, func(abandoned bool) { cancelled <- abandoned })
		waiting <- observation{scope, state}
		<-releaseWait
	}
	type outcome struct{ completed, ok bool }
	caller := make(chan outcome, 1)
	go func() {
		result, completed := nagiTaskParent(parent)
		caller <- outcome{completed, result.ok}
	}()
	seen := waitParentEvent(t, "first actual finish wait", waiting)
	waitParentEvent(t, "generated child timer", timerStarted)
	select {
	case <-cancelled:
		t.Fatal("discard or finish requested cancellation by itself")
	default:
	}
	publication := taskscope.TaskPublication(seen.scope)
	pending := fixtureMode == "parent_finish_pending"
	normal := fixtureMode == "parent_finish_normal"
	if pending {
		close(releaseWait)
		cancel()
		if !waitParentEvent(t, "owner cancellation of discarded child", cancelled) {
			t.Fatal("discarded output was not abandoned")
		}
		waitParentEvent(t, "child timer observing cancellation", interrupted)
		waitParentEvent(t, "blocked child cleanup", cleanupEntered)
	} else {
		waitParentEvent(t, "child cleanup after normal timer expiry", cleanupEntered)
		select {
		case <-cancelled:
			t.Fatal("finish cancelled child before cleanup")
		default:
		}
		close(releaseCleanup)
		waitParentEvent(t, "publication before recording", publication)
		if !normal {
			cancel()
		}
		close(releaseWait)
	}

	got := waitParentEvent(t, "parent exit", caller)
	if got.completed != normal || got.ok != normal {
		t.Fatalf("parent exit = %+v, normally completed=%t", got, normal)
	}
	if pending {
		if ready, recorded, retained := seen.state(); ready || recorded || !retained {
			t.Fatalf("blocked cleanup: ready=%t recorded=%t retained=%t", ready, recorded, retained)
		}
		close(releaseCleanup)
		waitParentEvent(t, "publication after cleanup", publication)
	}
	if !normal {
		if ready, recorded, retained := seen.state(); !ready || recorded || !retained {
			t.Fatalf("before independent finish: ready=%t recorded=%t retained=%t", ready, recorded, retained)
		}
		finished := make(chan error, 1)
		go func() { finished <- seen.scope.Finish() }()
		if err := waitParentEvent(t, "independent finish", finished); err != nil {
			t.Fatalf("finish after parent exit: %v", err)
		}
	}
	if ready, recorded, retained := seen.state(); ready || !recorded || retained {
		t.Fatalf("after finish: ready=%t recorded=%t retained=%t", ready, recorded, retained)
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
