package main

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"reflect"
	"testing"
)

func TestScalarWriterMatchesRust(t *testing.T) {
	raw, err := os.ReadFile("rust-chunks.json")
	if err != nil {
		t.Fatal(err)
	}
	var expected [][]string
	if err := json.Unmarshal(raw, &expected); err != nil {
		t.Fatal(err)
	}
	values := []any{"凪<&>\u2028\u2029\"\\\x00\x1f\b\f\n\r\t/end", "", true, int64(-1 << 63), uint64(1<<64 - 1)}
	if len(expected) != len(values) {
		t.Fatal("unexpected Rust oracle count")
	}
	for i, value := range values {
		var chunks []string
		err := nagiJSONWriteScalar(func(chunk string) (int, error) {
			chunks = append(chunks, chunk)
			return len(chunk), nil
		}, value)
		if err != nil || !reflect.DeepEqual(chunks, expected[i]) {
			t.Fatalf("case %d: writes = %q, error = %v, Rust writes = %q", i, chunks, err, expected[i])
		}
	}
}

func TestScalarWriterStopsOnSinkFailure(t *testing.T) {
	refused := errors.New("fixture sink exhausted")
	calls := 0
	err := nagiJSONWriteScalar(func(chunk string) (int, error) {
		calls++
		if calls == 3 {
			return 0, refused
		}
		return len(chunk), nil
	}, "one\ntwo")
	if err != refused || calls != 3 {
		t.Fatalf("sink refusal = %v after %d writes", err, calls)
	}
	calls = 0
	err = nagiJSONWriteScalar(func(string) (int, error) {
		calls++
		return 0, nil
	}, "one")
	if err != io.ErrShortWrite || calls != 1 {
		t.Fatalf("short write = %v after %d writes", err, calls)
	}
}
