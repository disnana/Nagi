// Staged only for the generated Task connection probe. It is not part of HTTP Serve.
package main

import (
	"context"
	"errors"
	"sync"

	"nagi.internal/go-runtime/internal/taskscope"
)

type nagiTaskConnOwner struct {
	mu   sync.Mutex
	conn *nagiHTTPConn
}

func (owner *nagiTaskConnOwner) take() *nagiHTTPConn {
	if owner == nil {
		return nil
	}
	owner.mu.Lock()
	defer owner.mu.Unlock()
	conn := owner.conn
	owner.conn = nil
	return conn
}

func nagiTaskSpawnWithConn[T any](owner *nagiTaskConnOwner, scope *taskscope.Scope, work func(context.Context) (T, bool), beforeClose func()) (taskscope.Handle[T], error) {
	conn := owner.take()
	if conn == nil {
		return taskscope.Handle[T]{}, taskscope.ErrConsumed
	}
	// Before registration succeeds the spawning side owns conn. On success
	// only the child cleanup can close it; on error the kernel starts no child.
	handle, err := taskscope.SpawnInterruptible(scope, work, func() {
		defer func() {
			if err := conn.Close(); err != nil {
				panic(err)
			}
		}()
		if beforeClose != nil {
			beforeClose()
		}
	})
	if err != nil {
		return handle, errors.Join(err, conn.Close())
	}
	return handle, nil
}
