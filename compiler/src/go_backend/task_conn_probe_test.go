package main

import (
	"context"
	"errors"
	"io"
	"net"
	"sync/atomic"
	"testing"
	"time"

	"nagi.internal/go-runtime/internal/taskscope"
)

type countedTCPConn struct {
	net.Conn
	closes atomic.Int32
}

func (conn *countedTCPConn) Close() error {
	conn.closes.Add(1)
	return conn.Conn.Close()
}

func waitConnEvent[T any](t *testing.T, limit context.Context, name string, event <-chan T) T {
	t.Helper()
	select {
	case value := <-event:
		return value
	case <-limit.Done():
		t.Fatalf("waiting for %s: %v", name, limit.Err())
		var zero T
		return zero
	}
}

type connProbe struct {
	limit    context.Context
	listener *nagiHTTPListener
	peer     net.Conn
	original *countedTCPConn
	owner    *nagiTaskConnOwner
}

func newConnProbe(t *testing.T) connProbe {
	t.Helper()
	limit, cancel := context.WithTimeout(context.Background(), 8*time.Second)
	t.Cleanup(cancel)
	raw, err := net.ListenTCP("tcp4", &net.TCPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		t.Fatal(err)
	}
	listener := &nagiHTTPListener{Listener: raw, slots: make(chan struct{}, 1), done: make(chan struct{})}
	t.Cleanup(func() { _ = listener.Close() })
	deadline, _ := limit.Deadline()
	if err := raw.SetDeadline(deadline); err != nil {
		t.Fatal(err)
	}
	probe := connProbe{limit: limit, listener: listener}
	probe.peer = probe.dial(t)
	conn, err := listener.Accept()
	if err != nil {
		t.Fatal(err)
	}
	accepted := conn.(*nagiHTTPConn)
	probe.original = &countedTCPConn{Conn: accepted.Conn}
	accepted.Conn = probe.original
	owner := &nagiTaskConnOwner{conn: accepted}
	probe.owner = owner
	t.Cleanup(func() {
		if pending := owner.take(); pending != nil {
			_ = pending.Close()
		}
	})
	return probe
}

func (probe connProbe) dial(t *testing.T) net.Conn {
	t.Helper()
	var dialer net.Dialer
	peer, err := dialer.DialContext(probe.limit, "tcp4", probe.listener.Addr().String())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = peer.Close() })
	deadline, _ := probe.limit.Deadline()
	if err := peer.SetDeadline(deadline); err != nil {
		t.Fatal(err)
	}
	return peer
}

type connAcceptance struct {
	conn net.Conn
	err  error
}

func (probe connProbe) nextAccept() <-chan connAcceptance {
	accepted := make(chan connAcceptance)
	go func() {
		conn, err := probe.listener.Accept()
		select {
		case accepted <- connAcceptance{conn, err}:
		case <-probe.limit.Done():
			if conn != nil {
				_ = conn.Close()
			}
		}
	}()
	return accepted
}

func (probe connProbe) checkHeld(t *testing.T, next <-chan connAcceptance) {
	t.Helper()
	if slots, closes := len(probe.listener.slots), probe.original.closes.Load(); slots != 1 || closes != 0 {
		t.Fatalf("connection should own the only slot: slots=%d closes=%d", slots, closes)
	}
	select {
	case got := <-next:
		if got.conn != nil {
			_ = got.conn.Close()
		}
		t.Fatalf("accept returned with the original slot still held: %v", got.err)
	default:
	}
}

func (probe connProbe) checkReadmission(t *testing.T, next <-chan connAcceptance, peer net.Conn) {
	t.Helper()
	if n, err := probe.peer.Read(make([]byte, 1)); n != 0 || err != io.EOF {
		t.Fatalf("original peer read: n=%d err=%v, want EOF", n, err)
	}
	accepted := waitConnEvent(t, probe.limit, "next real accept", next)
	if accepted.err != nil {
		t.Fatal(accepted.err)
	}
	defer func() {
		if accepted.conn != nil {
			_ = accepted.conn.Close()
		}
	}()
	deadline, _ := probe.limit.Deadline()
	if err := accepted.conn.SetDeadline(deadline); err != nil {
		t.Fatal(err)
	}
	exchange := func(from, to net.Conn, sent byte) {
		t.Helper()
		if n, err := from.Write([]byte{sent}); n != 1 || err != nil {
			t.Fatalf("write: n=%d err=%v", n, err)
		}
		var received [1]byte
		if _, err := io.ReadFull(to, received[:]); err != nil || received[0] != sent {
			t.Fatalf("read: got %d err=%v, want %d", received[0], err, sent)
		}
	}
	exchange(peer, accepted.conn, 'a')
	exchange(accepted.conn, peer, 'b')
	conn := accepted.conn
	accepted.conn = nil
	if closeErr := conn.Close(); closeErr != nil {
		t.Fatal(closeErr)
	}
	if slots, closes := len(probe.listener.slots), probe.original.closes.Load(); slots != 0 || closes != 1 {
		t.Fatalf("after readmission: slots=%d original closes=%d", slots, closes)
	}
}

func TestCheckedTaskLowering(t *testing.T) {
	t.Run("cancel_generated_child", testConnCancellation)
	t.Run("rejected_spawn_closes_original", testConnSpawnFailure)
}

func testConnCancellation(t *testing.T) {
	probe := newConnProbe(t)
	parent, cancel := context.WithCancel(context.Background())
	defer cancel()
	timerStarted := make(chan struct{})
	interrupted := make(chan struct{})
	cleanupEntered := make(chan struct{})
	releaseCleanup := make(chan struct{})
	defer func() {
		select {
		case <-releaseCleanup:
		default:
			close(releaseCleanup)
		}
	}()
	nagiTaskSleepStarted = func() { close(timerStarted) }
	nagiTaskSleepInterrupted = func() { close(interrupted) }
	nagiTaskCleanup = func() { close(cleanupEntered); <-releaseCleanup }

	type observation struct {
		scope *taskscope.Scope
		state func() (bool, bool, bool)
	}
	waiting := make(chan observation, 1)
	nagiTaskFinishWaiting = func(scope *taskscope.Scope, handle taskscope.Handle[nagiResult[int64, int64]]) {
		waiting <- observation{scope, taskscope.ObserveTaskExit(scope, handle, func(bool) {})}
	}
	caller := make(chan bool, 1)
	go func() {
		_, completed := nagiTaskParent(parent, probe.owner)
		caller <- completed
	}()
	waitConnEvent(t, probe.limit, "generated timer start", timerStarted)
	seen := waitConnEvent(t, probe.limit, "parent Finish wait", waiting)
	publication := taskscope.TaskPublication(seen.scope)
	replacementScope := taskscope.New()
	if _, err := nagiTaskSpawnWithConn(probe.owner, replacementScope, unusedConnWork, nil); err != taskscope.ErrConsumed {
		t.Fatalf("duplicate move: %v", err)
	}
	if entries, _ := taskscope.TaskOutstanding(replacementScope); entries != 0 {
		t.Fatalf("duplicate move registered %d children", entries)
	}

	peer := probe.dial(t)
	next := probe.nextAccept()
	probe.checkHeld(t, next)
	cancel()
	waitConnEvent(t, probe.limit, "generated timer interruption", interrupted)
	waitConnEvent(t, probe.limit, "cleanup before Close", cleanupEntered)
	if completed := waitConnEvent(t, probe.limit, "parent exit", caller); completed {
		t.Fatal("cancelled parent completed normally")
	}
	probe.checkHeld(t, next)
	if ready, recorded, retained := seen.state(); ready || recorded || !retained {
		t.Fatalf("cleanup stopped: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}

	close(releaseCleanup)
	probe.checkReadmission(t, next, peer)
	waitConnEvent(t, probe.limit, "publication after Close", publication)
	finished := make(chan error, 1)
	go func() { finished <- seen.scope.Finish() }()
	if err := waitConnEvent(t, probe.limit, "scope drain", finished); err != nil {
		t.Fatal(err)
	}
	if ready, recorded, retained := seen.state(); ready || !recorded || retained {
		t.Fatalf("after drain: ready=%t recorded=%t retained=%t", ready, recorded, retained)
	}
	if entries, queued := taskscope.TaskOutstanding(seen.scope); entries != 0 || queued != 0 {
		t.Fatalf("after drain: entries=%d queued=%d", entries, queued)
	}
}

func unusedConnWork(context.Context) (int, bool) {
	panic("unexpected child start")
}

func testConnSpawnFailure(t *testing.T) {
	probe := newConnProbe(t)
	scope := taskscope.New()
	if err := scope.Finish(); err != nil {
		t.Fatal(err)
	}
	peer := probe.dial(t)
	next := probe.nextAccept()
	probe.checkHeld(t, next)
	if _, err := nagiTaskSpawnWithConn(probe.owner, scope, unusedConnWork, nil); !errors.Is(err, taskscope.ErrClosed) {
		t.Fatalf("spawn into closed scope: %v", err)
	}
	if entries, queued := taskscope.TaskOutstanding(scope); entries != 0 || queued != 0 {
		t.Fatalf("rejected spawn: entries=%d queued=%d", entries, queued)
	}
	if _, err := nagiTaskSpawnWithConn(probe.owner, scope, unusedConnWork, nil); err != taskscope.ErrConsumed {
		t.Fatalf("retry after upstream Close: %v", err)
	}
	probe.checkReadmission(t, next, peer)
}
