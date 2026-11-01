// Package taskscope is a private normal-success prototype for Nagi Task scopes.
// It is not copied into generated applications and does not expose a Nagi API.
package taskscope

import (
	"errors"
	"sync"
)

var (
	ErrClosed        = errors.New("task scope is closed")
	ErrInvalid       = errors.New("invalid task handle")
	ErrWrongScope    = errors.New("task handle belongs to another scope")
	ErrConsumed      = errors.New("task handle was already consumed")
	ErrTaskFailed    = errors.New("task callback did not return normally")
	ErrCleanupFailed = errors.New("task cleanup did not return normally")
	ErrUnconsumed    = errors.New("task scope has an unconsumed result")
)

type identity struct{ marker byte }

type receiptState uint8

const (
	receiptLive receiptState = iota
	receiptReceiving
	receiptReceived
	receiptDiscarded
)

type receiptView interface {
	doneChannel() <-chan struct{}
	isConsumed() bool
	isComplete() bool
}

type receipt[T any] struct {
	mu       sync.Mutex
	state    receiptState
	value    *T
	problem  error
	complete bool
	done     chan struct{}
}

func newReceipt[T any]() *receipt[T] {
	return &receipt[T]{done: make(chan struct{})}
}

func (r *receipt[T]) doneChannel() <-chan struct{} { return r.done }

func (r *receipt[T]) isConsumed() bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.state >= receiptReceiving
}

func (r *receipt[T]) isComplete() bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.complete
}

// publish is the last wrapper action after the callback and cleanup return.
// Closing done proves those boundaries returned; it does not claim that the Go
// runtime has physically destroyed the goroutine.
func (r *receipt[T]) publish(value T, usable bool, problem error) {
	r.mu.Lock()
	if usable && r.state != receiptDiscarded {
		r.value = &value
	}
	r.problem = problem
	r.complete = true
	close(r.done)
	r.mu.Unlock()
}

type entry struct {
	receipt receiptView
}

type Scope struct {
	mu       sync.Mutex
	identity *identity
	next     uint64
	closed   bool
	entries  map[uint64]entry
}

func New() *Scope {
	return &Scope{
		identity: &identity{marker: 1},
		next:     1,
		entries:  make(map[uint64]entry),
	}
}

type Handle[T any] struct {
	owner   *identity
	ticket  uint64
	receipt *receipt[T]
}

// Spawn registers the receipt before starting the child goroutine. Work runs
// once inside that goroutine; cleanup, when provided, runs after work returns.
func Spawn[T any](scope *Scope, work func() T, cleanup func()) (Handle[T], error) {
	if scope == nil || work == nil {
		return Handle[T]{}, ErrInvalid
	}
	receipt := newReceipt[T]()
	scope.mu.Lock()
	if scope.closed {
		scope.mu.Unlock()
		return Handle[T]{}, ErrClosed
	}
	if scope.next == 0 {
		scope.mu.Unlock()
		return Handle[T]{}, ErrClosed
	}
	ticket := scope.next
	scope.next++
	scope.entries[ticket] = entry{receipt: receipt}
	owner := scope.identity
	scope.mu.Unlock()

	go func() {
		var value T
		returned := false
		cleaned := false
		defer func() {
			if !returned {
				_ = recover()
				receipt.publish(value, false, ErrTaskFailed)
				return
			}
			if !cleaned {
				_ = recover()
				receipt.publish(value, false, ErrCleanupFailed)
				return
			}
			receipt.publish(value, true, nil)
		}()
		value = work()
		returned = true
		if cleanup != nil {
			cleanup()
		}
		cleaned = true
	}()

	return Handle[T]{owner: owner, ticket: ticket, receipt: receipt}, nil
}

func entryFor[T any](scope *Scope, handle Handle[T]) (*receipt[T], error) {
	if scope == nil || handle.owner == nil || handle.ticket == 0 || handle.receipt == nil {
		return nil, ErrInvalid
	}
	scope.mu.Lock()
	defer scope.mu.Unlock()
	if handle.owner != scope.identity {
		return nil, ErrWrongScope
	}
	entry, ok := scope.entries[handle.ticket]
	if !ok || entry.receipt != handle.receipt {
		return nil, ErrInvalid
	}
	return handle.receipt, nil
}

func (scope *Scope) retire(ticket uint64, receipt receiptView) {
	scope.mu.Lock()
	defer scope.mu.Unlock()
	entry, ok := scope.entries[ticket]
	if ok && entry.receipt == receipt && receipt.isComplete() && receipt.isConsumed() {
		delete(scope.entries, ticket)
	}
}

// Receive claims one handle, then waits for that child callback and cleanup.
// It never waits for an unrelated sibling.
func Receive[T any](scope *Scope, handle Handle[T]) (T, error) {
	var zero T
	receipt, problem := entryFor(scope, handle)
	if problem != nil {
		return zero, problem
	}
	receipt.mu.Lock()
	if receipt.state != receiptLive {
		receipt.mu.Unlock()
		return zero, ErrConsumed
	}
	receipt.state = receiptReceiving
	receipt.mu.Unlock()

	<-receipt.done
	receipt.mu.Lock()
	problem = receipt.problem
	if problem == nil && receipt.value != nil {
		zero = *receipt.value
		receipt.value = nil
	} else if problem == nil {
		problem = ErrTaskFailed
	}
	receipt.state = receiptReceived
	receipt.mu.Unlock()
	scope.retire(handle.ticket, receipt)
	return zero, problem
}

// Discard abandons only the output. It neither cancels nor detaches the child;
// Finish still waits for the callback and cleanup boundary.
func Discard[T any](scope *Scope, handle Handle[T]) error {
	receipt, problem := entryFor(scope, handle)
	if problem != nil {
		return problem
	}
	receipt.mu.Lock()
	if receipt.state != receiptLive {
		receipt.mu.Unlock()
		return ErrConsumed
	}
	receipt.state = receiptDiscarded
	receipt.value = nil
	complete := receipt.complete
	receipt.mu.Unlock()
	if complete {
		scope.retire(handle.ticket, receipt)
	}
	return nil
}

// Finish seals registration and waits for every registered callback and cleanup.
// Repeated calls are idempotent after all explicit receive/discard obligations
// have retired. An unconsumed result is an internal contract error: the public
// checker prevents that state before this prototype can be connected.
func (scope *Scope) Finish() error {
	if scope == nil {
		return ErrInvalid
	}
	scope.mu.Lock()
	scope.closed = true
	entries := make([]entry, 0, len(scope.entries))
	for _, entry := range scope.entries {
		entries = append(entries, entry)
	}
	scope.mu.Unlock()
	for _, entry := range entries {
		<-entry.receipt.doneChannel()
	}
	scope.mu.Lock()
	defer scope.mu.Unlock()
	for ticket, entry := range scope.entries {
		if entry.receipt.isComplete() && entry.receipt.isConsumed() {
			delete(scope.entries, ticket)
		}
	}
	if len(scope.entries) != 0 {
		return ErrUnconsumed
	}
	return nil
}
