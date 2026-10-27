package lifecycle

import (
	"context"
	"errors"
	"fmt"
	"runtime"
	"strings"
	"sync/atomic"
	"testing"
	"time"
	"unsafe"
)

type unformattablePanic struct{ formatted *atomic.Bool }

func (p unformattablePanic) String() string {
	p.formatted.Store(true)
	panic("panic payload was formatted")
}

func options() Options {
	return Options{Children: 64, Restarts: 5, Window: time.Second, RestartDelay: time.Millisecond, ShutdownTimeout: time.Second}
}

func group(t *testing.T, opts Options, cleanup func() error) *Group {
	t.Helper()
	g, err := New(opts, cleanup)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		defer cancel()
		_ = g.StopAndWait(ctx)
		if g.Snapshot().Phase != Complete {
			t.Error("test leaked a retained lifecycle obligation")
		}
	})
	return g
}

func register(t *testing.T, g *Group, name string, policy Policy, factory Factory) {
	t.Helper()
	if err := g.Register(name, policy, factory); err != nil {
		t.Fatal(err)
	}
}

func start(t *testing.T, g *Group) {
	t.Helper()
	if err := g.Start(); err != nil {
		t.Fatal(err)
	}
}

func wait(t *testing.T, g *Group) error {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	err := g.Wait(ctx)
	if err == ErrIncomplete {
		t.Fatal("lifecycle deadline exceeded")
	}
	return err
}

func signal(t *testing.T, ch <-chan struct{}) {
	t.Helper()
	select {
	case <-ch:
	case <-time.After(2 * time.Second):
		t.Fatal("barrier deadline exceeded")
	}
}

func until(t *testing.T, condition func() bool) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for !condition() {
		if time.Now().After(deadline) {
			t.Fatal("condition deadline exceeded")
		}
		time.Sleep(time.Millisecond)
	}
}

func assertComplete(t *testing.T, g *Group) {
	t.Helper()
	s := g.Snapshot()
	if s.Phase != Complete || s.ActiveRecords != 0 || s.PendingRestarts != 0 || s.CleanupActive {
		t.Fatalf("retained obligations after completion: %+v", s)
	}
	g.mu.Lock()
	defer g.mu.Unlock()
	for _, slot := range g.slots {
		if slot.factory != nil || slot.cancel != nil {
			t.Fatal("completed group retains user callback")
		}
	}
	if g.cleanup != nil || len(g.history) != 0 {
		t.Fatal("completed group retains cleanup or timers")
	}
}

func TestRegistrationAndColdStart(t *testing.T) {
	// A short slice must not keep an unrelated multi-MiB source allocation.
	large := strings.Repeat("x", 8<<20)
	name := large[4<<20 : (4<<20)+12]
	named := group(t, options(), nil)
	register(t, named, name, Temporary, func(context.Context) error { return nil })
	stored := named.slots[0].name
	base, pointer := uintptr(unsafe.Pointer(unsafe.StringData(large))), uintptr(unsafe.Pointer(unsafe.StringData(stored)))
	if stored != name || (pointer >= base && pointer-base < uintptr(len(large))) {
		t.Fatal("registered name retained the large source backing allocation")
	}
	runtime.KeepAlive(large)
	var called atomic.Int64
	g := group(t, options(), nil)
	register(t, g, "once", Temporary, func(context.Context) error { called.Add(1); return nil })
	if called.Load() != 0 || g.Snapshot().Starts != 0 {
		t.Fatal("registration executed a factory")
	}
	if g.Register("once", Temporary, func(context.Context) error { return nil }) == nil {
		t.Fatal("duplicate child accepted")
	}
	start(t, g)
	if err := wait(t, g); err != nil {
		t.Fatal(err)
	}
	if called.Load() != 1 || g.Snapshot().Generations[0] != 1 {
		t.Fatal("wrong generation count")
	}
	if g.Start() == nil || g.Register("late", Temporary, func(context.Context) error { return nil }) == nil {
		t.Fatal("completed registration reopened")
	}
	assertComplete(t, g)
	for _, opts := range []Options{{}, {Children: 65537, Restarts: 1, Window: time.Second, ShutdownTimeout: time.Second}} {
		if _, err := New(opts, nil); err == nil {
			t.Fatal("invalid limits accepted")
		}
	}
}

func TestQueuedWorkerFailureCannotBecomeLateCancellation(t *testing.T) {
	g := group(t, options(), nil)
	register(t, g, "queued", Temporary, func(ctx context.Context) error {
		if ctx.Err() != nil {
			t.Error("worker was canceled before completion boundary")
		}
		return context.Canceled // An uncanceled worker returning this is failure.
	})
	// Park the manager before it drains a real launch/completion, so the late
	// RequestStop is deterministic rather than a scheduler-dependent sleep.
	g.mu.Lock()
	g.started = true
	g.phase = Running
	g.mu.Unlock()
	g.launch(0)
	var result completion
	select {
	case result = <-g.finished:
	case <-time.After(2 * time.Second):
		t.Fatal("completion barrier deadline exceeded")
	}
	g.RequestStop()
	g.receive(result)
	go g.manage()
	if err := wait(t, g); err != context.Canceled {
		t.Fatalf("late stop erased worker failure: %v", err)
	}
	if s := g.Snapshot(); s.WorkerFailures != 1 || s.Starts != 1 || s.ScheduledRestarts != 0 {
		t.Fatalf("queued failure reclassified: %+v", s)
	}
	assertComplete(t, g)
}

func TestOneForOnePanicRestart(t *testing.T) {
	g := group(t, options(), nil)
	healthy := make(chan struct{})
	var attempts atomic.Int64
	var released atomic.Bool
	var formatted atomic.Bool
	register(t, g, "healthy", Temporary, func(ctx context.Context) error { close(healthy); <-ctx.Done(); return ctx.Err() })
	register(t, g, "faulty", Transient, func(context.Context) error {
		if attempts.Add(1) == 1 {
			defer released.Store(true)
			panic(unformattablePanic{&formatted})
		}
		if !released.Load() {
			return errors.New("restarted before child defer finished")
		}
		return nil
	})
	start(t, g)
	signal(t, healthy)
	until(t, func() bool { s := g.Snapshot(); return s.Starts == 3 && s.ActiveRecords == 1 })
	s := g.Snapshot()
	if s.Generations[0] != 1 || s.Generations[1] != 2 || s.Panics != 1 {
		t.Fatalf("one-for-one violated: %+v", s)
	}
	if formatted.Load() {
		t.Fatal("formatted arbitrary panic payload")
	}
	if err := g.StopAndWait(context.Background()); err != nil {
		t.Fatal(err)
	}
	assertComplete(t, g)
}

func TestTemporaryFailureIsStickyUntilHealthySiblingJoins(t *testing.T) {
	fault := errors.New("worker failed")
	healthy, fail := make(chan struct{}), make(chan struct{})
	g := group(t, options(), nil)
	register(t, g, "healthy", Temporary, func(ctx context.Context) error { close(healthy); <-ctx.Done(); return ctx.Err() })
	register(t, g, "failed", Temporary, func(context.Context) error { <-fail; return fault })
	start(t, g)
	signal(t, healthy)
	close(fail)
	until(t, func() bool { return g.Snapshot().WorkerFailures == 1 })
	if s := g.Snapshot(); s.Phase != Running || s.ActiveRecords != 1 || s.ScheduledRestarts != 0 {
		t.Fatalf("healthy sibling stopped: %+v", s)
	}
	if err := g.StopAndWait(context.Background()); err == nil || err.Error() != fault.Error() {
		t.Fatalf("lost sticky worker failure: %v", err)
	}
	assertComplete(t, g)
}

func TestBusinessReplyIsNotAWorkerFailure(t *testing.T) {
	// This models a caller receiving a business Err; it does not implement
	// Actor/Turn or a public reply protocol. The worker itself returns success.
	business := errors.New("business rejected")
	reply := make(chan error, 1)
	g := group(t, options(), nil)
	register(t, g, "business", Transient, func(context.Context) error { reply <- business; return nil })
	start(t, g)
	if err := wait(t, g); err != nil {
		t.Fatal(err)
	}
	if <-reply != business {
		t.Fatal("business result changed")
	}
	if s := g.Snapshot(); s.Starts != 1 || s.WorkerFailures != 0 || s.ScheduledRestarts != 0 {
		t.Fatalf("business result restarted worker: %+v", s)
	}
	assertComplete(t, g)
}

func TestPermanentCompletionUsesGlobalRestartBudget(t *testing.T) {
	opts := options()
	opts.Restarts = 2
	g := group(t, opts, nil)
	register(t, g, "always", Permanent, func(context.Context) error { return nil })
	start(t, g)
	if err := wait(t, g); err != ErrIntensity {
		t.Fatalf("wrong terminal cause: %v", err)
	}
	if s := g.Snapshot(); s.Starts != 3 || s.ScheduledRestarts != 2 || s.WorkerFailures != 0 {
		t.Fatalf("wrong budget: %+v", s)
	}
	assertComplete(t, g)
}

func TestRestartBudgetIsSharedAndExactBoundaryExpires(t *testing.T) {
	now := time.Now()
	if n := len(expire([]time.Time{now.Add(-time.Second), now.Add(-time.Second + time.Nanosecond)}, now, time.Second)); n != 1 {
		t.Fatalf("wrong sliding-window boundary: %d", n)
	}
	opts := options()
	opts.Restarts = 1
	opts.RestartDelay = time.Second
	g := group(t, opts, nil)
	first, second := make(chan struct{}), make(chan struct{})
	register(t, g, "first", Transient, func(context.Context) error { <-first; return errors.New("first") })
	register(t, g, "second", Transient, func(ctx context.Context) error {
		select {
		case <-second:
			return errors.New("second")
		case <-ctx.Done():
			return ctx.Err()
		}
	})
	start(t, g)
	close(first)
	until(t, func() bool { return g.Snapshot().ScheduledRestarts == 1 })
	close(second)
	if err := wait(t, g); err != ErrIntensity {
		t.Fatalf("global budget not enforced: %v", err)
	}
	if s := g.Snapshot(); s.Starts != 2 || s.PendingRestarts != 0 {
		t.Fatalf("unexpected restart: %+v", s)
	}
	assertComplete(t, g)
}

func TestStopCancelsPendingRestartAndNeverRestartsCancellation(t *testing.T) {
	opts := options()
	opts.RestartDelay = time.Second
	g := group(t, opts, nil)
	register(t, g, "pending", Permanent, func(context.Context) error { return nil })
	start(t, g)
	until(t, func() bool { return g.Snapshot().PendingRestarts == 1 })
	if err := g.StopAndWait(context.Background()); err != nil {
		t.Fatal(err)
	}
	if s := g.Snapshot(); s.Starts != 1 || s.PendingRestarts != 0 {
		t.Fatalf("restart raced stop: %+v", s)
	}
	assertComplete(t, g)
	g2 := group(t, options(), nil)
	ready := make(chan struct{})
	register(t, g2, "cancel", Permanent, func(ctx context.Context) error { close(ready); <-ctx.Done(); return ctx.Err() })
	start(t, g2)
	signal(t, ready)
	if err := g2.StopAndWait(context.Background()); err != nil {
		t.Fatal(err)
	}
	if s := g2.Snapshot(); s.Starts != 1 || s.ScheduledRestarts != 0 || s.WorkerFailures != 0 {
		t.Fatalf("cancellation restarted worker: %+v", s)
	}
	assertComplete(t, g2)
}

func TestStopRacesRestartRepeatedly(t *testing.T) {
	for iteration := 0; iteration < 32; iteration++ {
		opts := options()
		opts.RestartDelay = 0
		opts.Restarts = 65536
		g := group(t, opts, nil)
		entered := make(chan struct{})
		var once atomic.Bool
		register(t, g, "race", Permanent, func(context.Context) error {
			if once.CompareAndSwap(false, true) {
				close(entered)
			}
			return nil
		})
		start(t, g)
		signal(t, entered)
		if err := g.StopAndWait(context.Background()); err != nil {
			t.Fatal(err)
		}
		before := g.Snapshot().Starts
		g.RequestStop()
		if g.Snapshot().Starts != before {
			t.Fatal("completed group restarted")
		}
		assertComplete(t, g)
	}
}

func TestBlockedWorkerRetainsRecordUntilActualReturn(t *testing.T) {
	opts := options()
	opts.ShutdownTimeout = 5 * time.Millisecond
	var cleaned atomic.Int64
	g := group(t, opts, func() error { cleaned.Add(1); return nil })
	entered, release := make(chan struct{}), make(chan struct{})
	defer close(release)
	register(t, g, "noncooperative", Temporary, func(context.Context) error { close(entered); <-release; return nil })
	start(t, g)
	signal(t, entered)
	if err := g.StopAndWait(context.Background()); err != ErrIncomplete {
		t.Fatalf("premature shutdown success: %v", err)
	}
	if s := g.Snapshot(); s.ActiveRecords != 1 || s.Phase != Stopping || cleaned.Load() != 0 {
		t.Fatalf("lost live obligation: %+v", s)
	}
	// Release without closing: deferred close remains a failure-path escape.
	release <- struct{}{}
	if err := wait(t, g); err != nil {
		t.Fatal(err)
	}
	if cleaned.Load() != 1 {
		t.Fatal("cleanup did not run exactly once")
	}
	assertComplete(t, g)
}

func TestBlockedCleanupRunsOutsideLocksAndRetainsCompletion(t *testing.T) {
	opts := options()
	opts.ShutdownTimeout = 5 * time.Millisecond
	entered, release := make(chan struct{}), make(chan struct{})
	defer close(release)
	var g *Group
	g = group(t, opts, func() error { close(entered); _ = g.Snapshot(); g.RequestStop(); <-release; return nil })
	register(t, g, "once", Temporary, func(context.Context) error { return nil })
	start(t, g)
	signal(t, entered)
	if err := g.StopAndWait(context.Background()); err != ErrIncomplete {
		t.Fatalf("blocked cleanup reported complete: %v", err)
	}
	if s := g.Snapshot(); s.Phase != Cleaning || !s.CleanupActive || s.ActiveRecords != 0 {
		t.Fatalf("cleanup obligation lost: %+v", s)
	}
	release <- struct{}{}
	if err := wait(t, g); err != nil {
		t.Fatal(err)
	}
	assertComplete(t, g)
}

func TestDeferredCleanupPrecedesJoinAndFailureSurvivesStop(t *testing.T) {
	entered, release := make(chan struct{}), make(chan struct{})
	defer close(release)
	fault := errors.New("racing worker failure")
	var released atomic.Bool
	g := group(t, options(), func() error {
		if !released.Load() {
			return errors.New("cleanup preceded child defer")
		}
		return nil
	})
	register(t, g, "defer", Temporary, func(context.Context) error {
		defer func() { close(entered); <-release; released.Store(true) }()
		return fault
	})
	start(t, g)
	signal(t, entered)
	g.RequestStop()
	if g.Snapshot().ActiveRecords != 1 {
		t.Fatal("joined before child defer")
	}
	release <- struct{}{}
	if err := wait(t, g); err == nil || err.Error() != fault.Error() {
		t.Fatalf("lost racing failure: %v", err)
	}
	assertComplete(t, g)
}

func TestCleanupPanicAndUnstartedStop(t *testing.T) {
	var calls atomic.Int64
	g := group(t, options(), func() error { calls.Add(1); panic("cleanup") })
	register(t, g, "never", Temporary, func(context.Context) error { t.Error("stopped registration ran"); return nil })
	if err := g.StopAndWait(context.Background()); err != ErrCleanupPanicked {
		t.Fatalf("cleanup panic not retained: %v", err)
	}
	if calls.Load() != 1 || g.Snapshot().Starts != 0 {
		t.Fatal("unstarted lifecycle changed")
	}
	assertComplete(t, g)
	bounded := group(t, options(), nil)
	register(t, bounded, "bounded", Temporary, func(context.Context) error { return errors.New(strings.Repeat("凪", 1000)) })
	start(t, bounded)
	err := wait(t, bounded)
	if err == nil || len(err.Error()) != 1023 || err.Error() != strings.Repeat("凪", 341) {
		t.Fatal("worker error text was not UTF-8 bounded")
	}
	assertComplete(t, bounded)
}

func BenchmarkLifecycle(b *testing.B) {
	for _, children := range []int{64, 1024} {
		b.Run(fmt.Sprint(children), func(b *testing.B) {
			for iteration := 0; iteration < b.N; iteration++ {
				opts := options()
				opts.Children = children
				g, _ := New(opts, nil)
				ready := make(chan struct{}, children)
				for index := 0; index < children; index++ {
					_ = g.Register(fmt.Sprint(index), Temporary, func(ctx context.Context) error { ready <- struct{}{}; <-ctx.Done(); return ctx.Err() })
				}
				_ = g.Start()
				for index := 0; index < children; index++ {
					<-ready
				}
				if err := g.StopAndWait(context.Background()); err != nil {
					b.Fatal(err)
				}
			}
		})
	}
}
