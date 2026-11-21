package main

import (
	"testing"
	"time"

	"nagi.internal/go-runtime/internal/taskscope"
)

func TestGeneratedTaskProbe(t *testing.T) {
	for _, fixture := range []struct {
		name string
		mode int64
	}{
		{"value", 0},
		{"business_error", 1},
		{"panic", 2},
	} {
		t.Run(fixture.name, func(t *testing.T) {
			type observation struct {
				received probeReceived
				cleaned  bool
			}
			scope := taskscope.New()
			cleaning := make(chan struct{})
			release := make(chan struct{})
			cleaned := make(chan struct{})
			observed := make(chan observation, 1)
			finished := make(chan nagiResult[nagiUnit, nagiError], 1)
			defer func() {
				select {
				case <-release:
				default:
					close(release)
				}
			}()
			go func() {
				received := taskProbeReceive(scope, fixture.mode, func() {
					close(cleaning)
					<-release
					close(cleaned)
				})
				result := observation{received: received}
				select {
				case <-cleaned:
					result.cleaned = true
				default:
				}
				observed <- result
				finished <- taskProbeFinish(scope)
			}()
			select {
			case <-cleaning:
			case <-time.After(5 * time.Second):
				t.Fatal("child did not reach cleanup")
			}
			select {
			case got := <-observed:
				t.Fatalf("receive returned before cleanup: %+v", got)
			default:
			}
			close(release)
			var got observation
			select {
			case got = <-observed:
			case <-time.After(5 * time.Second):
				t.Fatal("receive did not return after cleanup")
			}
			if !got.cleaned {
				t.Fatal("receive returned before cleanup completed")
			}
			var exit nagiResult[nagiUnit, nagiError]
			select {
			case exit = <-finished:
			case <-time.After(5 * time.Second):
				t.Fatal("scope did not finish after cleanup")
			}
			if fixture.mode == 2 {
				if got.received.ok || got.received.problem != (probeTaskFailure{probePanicked, "child panicked"}) {
					t.Fatalf("receive panic = %+v", got.received)
				}
				if exit.ok || exit.problem != (nagiError{kind: probeInternal, message: "child panicked"}) {
					t.Fatalf("scope panic = %+v", exit)
				}
				return
			}
			if !got.received.ok || !exit.ok {
				t.Fatalf("body result became scope fault: receive=%+v exit=%+v", got.received, exit)
			}
			inner := got.received.value
			if fixture.mode == 0 && (!inner.ok || inner.value != 17) {
				t.Fatalf("typed result = %+v, want Ok(17)", inner)
			}
			if fixture.mode == 1 && (inner.ok || inner.problem != 22) {
				t.Fatalf("typed result = %+v, want Err(22)", inner)
			}
		})
	}
}
