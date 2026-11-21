// Copied only into the generated body-panic test's private kernel package.
package taskscope

// ObserveTaskExit observes cancellation invocation and the actual scope
// record. The wrapper still delegates cancellation to the original owner.
func ObserveTaskExit[T any](scope *Scope, handle Handle[T], onCancel func(bool)) func() (ready, recorded, retained bool) {
	scope.mu.Lock()
	entry := scope.entries[handle.ticket]
	cancel := entry.cancel
	entry.cancel = func() {
		handle.receipt.mu.Lock()
		abandoned := handle.receipt.state == receiptDiscarded && handle.receipt.value == nil
		handle.receipt.mu.Unlock()
		onCancel(abandoned)
		cancel()
	}
	scope.mu.Unlock()
	return func() (bool, bool, bool) {
		scope.mu.Lock()
		defer scope.mu.Unlock()
		_, retained := scope.entries[handle.ticket]
		return entry.ready, entry.recorded, retained
	}
}

func TaskPublication(scope *Scope) <-chan struct{} {
	scope.mu.Lock()
	defer scope.mu.Unlock()
	return scope.notify
}

// TaskOutstanding counts retained entries and queued completion notifications.
func TaskOutstanding(scope *Scope) (entries, queued int) {
	scope.mu.Lock()
	defer scope.mu.Unlock()
	return len(scope.entries), len(scope.ready)
}
