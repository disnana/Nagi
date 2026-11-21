package main

import (
	"testing"
	"time"
)

func TestCheckedTaskLowering(t *testing.T) {
	entered := make(chan struct{})
	release := make(chan struct{})
	cleaned := make(chan struct{})
	childDone := make(chan (<-chan struct{}), 1)
	nagiTaskChildDone = func(done <-chan struct{}) { childDone <- done }
	if fixtureMode == "body_error_before_await" {
		nagiTaskBeforeBodyTry = func() { <-entered }
	}
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
	type scopeExit struct {
		result  nagiResult[nagiUnit, nagiError]
		cleaned bool
	}
	finished := make(chan scopeExit, 1)
	go func() {
		exit := scopeExit{result: nagiMain().await()}
		select {
		case <-cleaned:
			exit.cleaned = true
		default:
		}
		finished <- exit
	}()
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
	if fixtureMode == "body_error_before_await" {
		// The hook held the body until the child entered panic cleanup.
		// The cleanup is still blocked. Since the child's
		// completion cannot be recorded, ctx.Done can only be body teardown.
		done := <-childDone
		select {
		case <-done:
		case early := <-finished:
			t.Fatalf("body returned before requesting child teardown: %+v", early)
		case <-time.After(5 * time.Second):
			t.Fatal("body did not request child teardown")
		}
		select {
		case early := <-finished:
			t.Fatalf("body returned while cleanup was blocked: %+v", early)
		default:
		}
	}
	close(release)

	var finishedScope scopeExit
	select {
	case finishedScope = <-finished:
	case <-time.After(5 * time.Second):
		t.Fatal("scope never finished")
	}
	if !finishedScope.cleaned {
		t.Fatal("scope exit preceded child cleanup")
	}
	exit := finishedScope.result
	if fixtureMode == "body_error_before_await" || fixtureMode == "body_error_after_await" {
		if exit.ok || exit.problem != (nagiError{kind: nagiInvalid, message: "body sentinel"}) {
			t.Fatalf("body error was replaced: %+v", exit)
		}
		if fixtureMode == "body_error_before_await" {
			select {
			case result := <-received:
				t.Fatalf("received a task after body error: %+v", result)
			default:
			}
			return
		}
	}

	var result observation
	select {
	case result = <-received:
	case <-time.After(5 * time.Second):
		t.Fatal("receive never recorded an outcome")
	}
	if !result.cleaned {
		t.Fatal("result preceded child cleanup")
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
	case "panic", "body_error_after_await":
		if result.value.ok || result.value.problem != (nagiTaskFailure{nagiTaskPanicked, "child panicked"}) {
			t.Fatalf("panic receive=%+v", result)
		}
		if fixtureMode == "panic" && (exit.ok || exit.problem != (nagiError{kind: nagiTaskInternal, message: "child panicked"})) {
			t.Fatalf("panic scope exit=%+v", exit)
		}
	default:
		t.Fatalf("unknown case %q", fixtureMode)
	}
}
