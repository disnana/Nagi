// Package taskscope is a private Task-scope prototype for the Go backend.
// It is not copied into generated applications and exposes no Nagi API.
package taskscope

import (
	"context"
	"errors"
	"sync"
)

var (
	ErrClosed          = errors.New("task scope is closed")
	ErrInvalid         = errors.New("invalid task handle")
	ErrWrongScope      = errors.New("task handle belongs to another scope")
	ErrConsumed        = errors.New("task handle was already consumed")
	ErrTaskPanicked    = errors.New("task callback panicked")
	ErrTaskGoexit      = errors.New("task callback exited without returning")
	ErrCleanupPanicked = errors.New("task cleanup panicked")
	ErrCleanupGoexit   = errors.New("task cleanup exited without returning")
	ErrMissingOutput   = errors.New("task completed without an output")
	ErrUnconsumed      = errors.New("task scope has an unconsumed result")
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
	isRetirable() bool
	isUnconsumed() bool
	completionFaults() []error
}

type receipt[T any] struct {
	mu       sync.Mutex
	state    receiptState
	value    *T
	faults   []error
	complete bool
	done     chan struct{}
}

func newReceipt[T any]() *receipt[T] {
	return &receipt[T]{done: make(chan struct{})}
}

func (r *receipt[T]) doneChannel() <-chan struct{} { return r.done }

func (r *receipt[T]) isRetirable() bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.state == receiptReceived || r.state == receiptDiscarded
}

func (r *receipt[T]) isUnconsumed() bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.state == receiptLive
}

func (r *receipt[T]) completionFaults() []error {
	r.mu.Lock()
	defer r.mu.Unlock()
	return append([]error(nil), r.faults...)
}

// publish is the last wrapper action after the callback and cleanup boundaries.
// Closing done proves those boundaries returned; it does not claim that the Go
// runtime has physically destroyed the goroutine.
func (r *receipt[T]) publish(value T, usable bool, faults []error) {
	r.mu.Lock()
	if usable && r.state != receiptDiscarded {
		r.value = &value
	}
	r.faults = append([]error(nil), faults...)
	r.complete = true
	close(r.done)
	r.mu.Unlock()
}

type entry struct {
	receipt         receiptView
	cancel          context.CancelFunc
	ready           bool
	recorded        bool
	requestedCancel bool
}

type Scope struct {
	mu       sync.Mutex
	identity *identity
	next     uint64
	closed   bool
	entries  map[uint64]*entry
	ready    []uint64
	notify   chan struct{}
	primary  error
	related  []error
}

func New() *Scope {
	return &Scope{
		identity: &identity{marker: 1},
		next:     1,
		entries:  make(map[uint64]*entry),
		notify:   make(chan struct{}),
	}
}

type Handle[T any] struct {
	owner   *identity
	ticket  uint64
	receipt *receipt[T]
}

func classifyBoundary(recovered any, panicked, goexit error) error {
	if recovered == nil {
		return goexit
	}
	return panicked
}

// runChild uses nested deferred boundaries so explicit cleanup runs once after
// normal return, panic, or Goexit, and final publication still happens if the
// cleanup itself panics or calls Goexit. Arbitrary panic payloads are discarded
// without formatting or retaining them.
func runChild[T any](ctx context.Context, work func(context.Context) T, cleanup func(), publish func(T, bool, []error)) {
	var value T
	var faults []error
	workReturned := false
	cleanupReturned := cleanup == nil
	defer func() {
		publish(value, workReturned && cleanupReturned && len(faults) == 0, faults)
	}()
	defer func() {
		if !workReturned {
			faults = append(faults, classifyBoundary(recover(), ErrTaskPanicked, ErrTaskGoexit))
		}
		if cleanup == nil {
			return
		}
		defer func() {
			if !cleanupReturned {
				faults = append(faults, classifyBoundary(recover(), ErrCleanupPanicked, ErrCleanupGoexit))
			}
		}()
		cleanup()
		cleanupReturned = true
	}()
	value = work(ctx)
	workReturned = true
}

// Spawn registers the receipt and cancellation owner before starting the child
// goroutine. Cancellation is advisory: a normal callback return remains T even
// when ctx is cancelled, including T values that contain context.Canceled.
func Spawn[T any](scope *Scope, work func(context.Context) T, cleanup func()) (Handle[T], error) {
	if scope == nil || work == nil {
		return Handle[T]{}, ErrInvalid
	}
	receipt := newReceipt[T]()
	ctx, cancel := context.WithCancel(context.Background())
	scope.mu.Lock()
	if scope.closed {
		scope.mu.Unlock()
		cancel()
		return Handle[T]{}, ErrClosed
	}
	if scope.next == 0 {
		scope.mu.Unlock()
		cancel()
		return Handle[T]{}, ErrClosed
	}
	ticket := scope.next
	scope.next++
	requestedCancel := scope.primary != nil
	entryCancel := cancel
	if requestedCancel {
		entryCancel = nil
	}
	scope.entries[ticket] = &entry{
		receipt:         receipt,
		cancel:          entryCancel,
		requestedCancel: requestedCancel,
	}
	owner := scope.identity
	scope.mu.Unlock()
	if requestedCancel {
		cancel()
	}

	go func() {
		runChild(ctx, work, cleanup, func(value T, usable bool, faults []error) {
			receipt.publish(value, usable, faults)
			scope.publish(ticket, receipt)
		})
	}()

	return Handle[T]{owner: owner, ticket: ticket, receipt: receipt}, nil
}

// publish only queues immutable completion. Child failure does not interrupt an
// executing scope body; Receive, Finish, or Cancel records the completion.
func (scope *Scope) publish(ticket uint64, receipt receiptView) {
	scope.mu.Lock()
	entry, ok := scope.entries[ticket]
	if !ok || entry.receipt != receipt || entry.ready || entry.recorded {
		scope.mu.Unlock()
		panic("invalid private task completion")
	}
	entry.ready = true
	scope.ready = append(scope.ready, ticket)
	close(scope.notify)
	scope.notify = make(chan struct{})
	scope.mu.Unlock()
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

func (scope *Scope) retireLocked(ticket uint64, receipt receiptView) {
	entry, ok := scope.entries[ticket]
	if ok && entry.receipt == receipt && entry.recorded && receipt.isRetirable() {
		delete(scope.entries, ticket)
	}
}

func (scope *Scope) retire(ticket uint64, receipt receiptView) {
	scope.mu.Lock()
	scope.retireLocked(ticket, receipt)
	scope.mu.Unlock()
}

func runCancels(cancels []context.CancelFunc) {
	for _, cancel := range cancels {
		cancel()
	}
}

func (scope *Scope) requestCancellationLocked() []context.CancelFunc {
	var cancels []context.CancelFunc
	for _, entry := range scope.entries {
		if entry.recorded || entry.requestedCancel || entry.cancel == nil {
			continue
		}
		entry.requestedCancel = true
		cancels = append(cancels, entry.cancel)
		entry.cancel = nil
	}
	return cancels
}

// recordReady records one completion in publication order. The first observed
// fault becomes sticky and requests sibling cancellation in the same locked
// transition; CancelFuncs run after unlocking and before further drain waits.
func (scope *Scope) recordReady() bool {
	var cancels []context.CancelFunc
	scope.mu.Lock()
	if len(scope.ready) == 0 {
		scope.mu.Unlock()
		return false
	}
	ticket := scope.ready[0]
	copy(scope.ready, scope.ready[1:])
	scope.ready = scope.ready[:len(scope.ready)-1]
	entry := scope.entries[ticket]
	if entry == nil || !entry.ready || entry.recorded {
		scope.mu.Unlock()
		panic("invalid private task accounting")
	}
	entry.ready = false
	entry.recorded = true
	faults := entry.receipt.completionFaults()
	if entry.cancel != nil {
		cancels = append(cancels, entry.cancel)
		entry.cancel = nil
	}
	if len(faults) != 0 {
		if scope.primary == nil {
			scope.primary = faults[0]
			scope.related = append(scope.related, faults[1:]...)
			cancels = append(cancels, scope.requestCancellationLocked()...)
		} else {
			scope.related = append(scope.related, faults...)
		}
	}
	scope.retireLocked(ticket, entry.receipt)
	scope.mu.Unlock()
	runCancels(cancels)
	return true
}

func (scope *Scope) allRecordedLocked() bool {
	for _, entry := range scope.entries {
		if !entry.recorded {
			return false
		}
	}
	return true
}

func (scope *Scope) drain() {
	for {
		scope.mu.Lock()
		hasReady := len(scope.ready) != 0
		if scope.allRecordedLocked() {
			scope.mu.Unlock()
			return
		}
		notify := scope.notify
		scope.mu.Unlock()
		if hasReady {
			scope.recordReady()
			continue
		}
		<-notify
	}
}

func consumePrimary[T any](scope *Scope, ticket uint64, receipt *receipt[T]) error {
	scope.drain()
	scope.mu.Lock()
	primary := scope.primary
	receipt.mu.Lock()
	receipt.value = nil
	receipt.state = receiptReceived
	receipt.mu.Unlock()
	scope.retireLocked(ticket, receipt)
	scope.mu.Unlock()
	return primary
}

// Receive claims one handle, then records completions until its target joins or
// a sticky scope fault is observed. A sticky fault drains every child before it
// is returned and never flattens or inspects a business T value.
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

	for {
		scope.mu.Lock()
		if scope.primary != nil {
			scope.mu.Unlock()
			return zero, consumePrimary(scope, handle.ticket, receipt)
		}
		entry, ok := scope.entries[handle.ticket]
		if !ok || entry.receipt != receipt {
			scope.mu.Unlock()
			return zero, ErrInvalid
		}
		if entry.recorded {
			receipt.mu.Lock()
			if len(receipt.faults) != 0 {
				receipt.mu.Unlock()
				scope.mu.Unlock()
				return zero, consumePrimary(scope, handle.ticket, receipt)
			}
			if receipt.value == nil {
				receipt.state = receiptReceived
				receipt.mu.Unlock()
				scope.primary = ErrMissingOutput
				cancels := scope.requestCancellationLocked()
				scope.mu.Unlock()
				runCancels(cancels)
				return zero, consumePrimary(scope, handle.ticket, receipt)
			}
			zero = *receipt.value
			receipt.value = nil
			receipt.state = receiptReceived
			receipt.mu.Unlock()
			scope.retireLocked(handle.ticket, receipt)
			scope.mu.Unlock()
			return zero, nil
		}
		hasReady := len(scope.ready) != 0
		notify := scope.notify
		scope.mu.Unlock()
		if hasReady {
			scope.recordReady()
			continue
		}
		<-notify
	}
}

// Discard abandons only the output. It neither records completion nor cancels
// or detaches the child; Finish remains responsible for observing and draining.
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
	receipt.mu.Unlock()
	scope.retire(handle.ticket, receipt)
	return nil
}

func (scope *Scope) seal() {
	scope.mu.Lock()
	scope.closed = true
	scope.mu.Unlock()
}

// RequestOwnerTeardown seals registration and requests cooperative cancellation
// exactly once per live child. It deliberately does not wait: the checked
// owner-teardown contract is only an abort-request boundary. Finish, Cancel, or
// FinishBodyError must still drain callback and cleanup completion explicitly.
func (scope *Scope) RequestOwnerTeardown() {
	if scope == nil {
		panic(ErrInvalid)
	}
	scope.mu.Lock()
	scope.closed = true
	cancels := scope.requestCancellationLocked()
	scope.mu.Unlock()
	runCancels(cancels)
}

// FinishBodyError models the checked scope body-error exit without flattening
// or formatting E. The original body error stays outside the Task fault state,
// owner teardown requests cancellation, and the exact value is returned only
// after every registered callback and cleanup boundary has completed. Public Go
// lowering and body-local output abandonment remain separate later stages.
func FinishBodyError[E any](scope *Scope, bodyError E) E {
	scope.RequestOwnerTeardown()
	scope.drain()
	return bodyError
}

// Finish seals registration, observes completions in publication order, starts
// cooperative sibling cancellation at the first observed fault, and drains all
// callback/cleanup boundaries. Sticky faults take precedence over unconsumed T.
func (scope *Scope) Finish() error {
	if scope == nil {
		return ErrInvalid
	}
	scope.seal()
	scope.drain()
	scope.mu.Lock()
	defer scope.mu.Unlock()
	if scope.primary != nil {
		return scope.primary
	}
	for _, entry := range scope.entries {
		if entry.receipt.isUnconsumed() {
			return ErrUnconsumed
		}
	}
	return nil
}

// Cancel seals the scope, requests cooperative cancellation, and drains every
// callback/cleanup boundary. Cancellation does not fabricate result consumption.
func (scope *Scope) Cancel() error {
	if scope == nil {
		return ErrInvalid
	}
	scope.RequestOwnerTeardown()
	scope.drain()
	scope.mu.Lock()
	defer scope.mu.Unlock()
	return scope.primary
}
