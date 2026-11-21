package main

import (
	"testing"
	"time"
)

func TestParentPermitRepetition(t *testing.T) {
	const rounds = 64
	deadline := time.Now().Add(10 * time.Second)
	permit := make(chan struct{}, 1)
	permit <- struct{}{}
	var counts permitRoundCounts
	for round := 0; round < rounds; round++ {
		if len(permit) != 1 {
			t.Fatalf("round %d/%d: available=%d before acquisition", round+1, rounds, len(permit))
		}
		runParentPermitRound(t, permit, deadline, &counts)
	}
	if counts != (permitRoundCounts{completions: rounds, retired: rounds, reacquired: rounds}) || len(permit) != 1 {
		t.Fatalf("mode=%s rounds=%d counts=%+v available=%d", fixtureMode, rounds, counts, len(permit))
	}
	t.Logf("mode=%s rounds=%d child_completions=%d retired_scopes=%d reacquired_permits=%d available=%d",
		fixtureMode, rounds, counts.completions, counts.retired, counts.reacquired, len(permit))
}
