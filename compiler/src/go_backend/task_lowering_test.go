package main

import (
	"testing"
	"time"
)

func TestCheckedTaskLowering(t *testing.T) {
	entered := make(chan struct{})
	release := make(chan struct{})
	cleaned := make(chan struct{})
	defer func() {
		select {
		case <-release:
		default:
			close(release)
		}
	}()
	nagiTaskCleanup = func() {
		close(entered)
		<-release
		close(cleaned)
	}
	type receipt = nagiResult[nagiResult[int64, int64], nagiTaskFailure]
	type observation struct {
		value   receipt
		cleaned bool
	}
	received := make(chan observation, 1)
	nagiTaskObserve = func(value receipt) {
		result := observation{value: value}
		select {
		case <-cleaned:
			result.cleaned = true
		default:
		}
		received <- result
	}
	finished := make(chan nagiResult[nagiUnit, nagiError], 1)
	go func() { finished <- nagiMain().await() }()
	select {
	case <-entered:
	case <-time.After(5 * time.Second):
		t.Fatal("child never entered cleanup")
	}
	select {
	case <-received:
		t.Fatal("receive returned before cleanup")
	default:
	}
	close(release)

	var result observation
	select {
	case result = <-received:
	case <-time.After(5 * time.Second):
		t.Fatal("receive never recorded an outcome")
	}
	if !result.cleaned {
		t.Fatal("result preceded child cleanup")
	}
	var exit nagiResult[nagiUnit, nagiError]
	select {
	case exit = <-finished:
	case <-time.After(5 * time.Second):
		t.Fatal("scope never finished")
	}
	switch fixtureMode {
	case "value":
		if !result.value.ok || !result.value.value.ok || result.value.value.value != 17 || !exit.ok {
			t.Fatalf("value receive=%+v exit=%+v", result, exit)
		}
	case "business_error":
		if !result.value.ok || result.value.value.ok || result.value.value.problem != 22 || !exit.ok {
			t.Fatalf("business error receive=%+v exit=%+v", result, exit)
		}
	case "panic":
		if result.value.ok || result.value.problem != (nagiTaskFailure{nagiTaskPanicked, "child panicked"}) {
			t.Fatalf("panic receive=%+v", result)
		}
		if exit.ok || exit.problem != (nagiError{kind: nagiTaskInternal, message: "child panicked"}) {
			t.Fatalf("panic scope exit=%+v", exit)
		}
	default:
		t.Fatalf("unknown case %q", fixtureMode)
	}
}
