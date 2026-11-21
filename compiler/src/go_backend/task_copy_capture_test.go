package main

import (
	"context"
	"testing"
	"time"
)

func TestGeneratedTaskCopyCapture(t *testing.T) {
	limit, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	started := make(chan struct{})
	rebound := make(chan struct{})
	release := make(chan struct{})
	defer func() {
		select {
		case <-release:
		default:
			close(release)
		}
	}()
	nagiTaskChildDone = func(<-chan struct{}) { close(started); <-release }
	nagiTaskCopyRebound = func() { close(rebound) }
	type receipt = nagiResult[nagiResult[int64, int64], nagiTaskFailure]
	received := make(chan receipt, 1)
	nagiTaskObserve = func(value receipt) { received <- value }
	returned := make(chan nagiResult[nagiUnit, nagiError], 1)
	go func() { returned <- nagiMain().await() }()
	for _, event := range []struct {
		name string
		done <-chan struct{}
	}{{"child before entering its Future", started}, {"parent reassignment", rebound}} {
		select {
		case <-event.done:
		case <-limit.Done():
			t.Fatalf("timed out waiting for %s", event.name)
		}
	}

	close(release)
	select {
	case got := <-received:
		if !got.ok || !got.value.ok || got.value.value != 17 {
			t.Fatalf("received %+v after parent reassignment, want Ok(Ok(17))", got)
		}
	case <-limit.Done():
		t.Fatal("timed out waiting for child result")
	}
	select {
	case exit := <-returned:
		if !exit.ok {
			t.Fatalf("scope failed: %+v", exit)
		}
	case <-limit.Done():
		t.Fatal("timed out waiting for scope exit")
	}
}
