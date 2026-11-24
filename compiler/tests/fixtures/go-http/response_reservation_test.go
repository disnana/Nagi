package main

import (
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

func TestNagiHTTPResponseReservationBoundaries(t *testing.T) {
	const media = "text/plain; charset=utf-8"
	const headerBytes = int64(len("Content-Type: " + media + "\r\nContent-Length: 3\r\nConnection: close\r\nVary: Origin\r\n"))
	for _, tc := range []struct {
		name   string
		limits nagiHTTPResponseLimits
		kind   uint8
	}{
		{"exact", nagiHTTPResponseLimits{bodyBytes: 3, managedBytes: headerBytes, managedCount: 4, live: 1, retained: 67}, 0},
		{"body", nagiHTTPResponseLimits{bodyBytes: 2, managedBytes: headerBytes, managedCount: 4, live: 1, retained: 67}, nagiInvalid},
		{"managed bytes", nagiHTTPResponseLimits{bodyBytes: 3, managedBytes: headerBytes - 1, managedCount: 4, live: 1, retained: 66}, nagiInvalid},
		{"managed count", nagiHTTPResponseLimits{bodyBytes: 3, managedBytes: headerBytes, managedCount: 3, live: 1, retained: 67}, nagiInvalid},
		{"retained", nagiHTTPResponseLimits{bodyBytes: 3, managedBytes: headerBytes, managedCount: 4, live: 1, retained: 66}, nagiBusy},
	} {
		t.Run(tc.name, func(t *testing.T) {
			pool := &nagiHTTPResponsePool{}
			context := nagiHTTPResponseContext{headers: http.Header{"Vary": {"Origin"}}, limits: tc.limits, pool: pool}
			context.limits.serializationWrites = 1
			filled := false
			result := nagiHTTPProduce(&context, nagiHTTPOK, media, 3, func(out *nagiHTTPFiniteWriter) error {
				filled = true
				if context.charge != 67 || pool.live != 1 || pool.retained != 67 {
					t.Fatal("body fill ran before the response was charged")
				}
				_, err := out.WriteString("abc")
				return err
			})
			if tc.kind != 0 {
				if result.ok || result.problem.kind != tc.kind || filled || context.charge != 0 || pool.live != 0 || pool.retained != 0 {
					t.Fatalf("refused response affected body production or quota: %+v", result)
				}
				return
			}
			if !result.ok || !filled || string(result.value.body) != "abc" {
				t.Fatalf("exact limit did not admit response: %+v", result)
			}
			if context.reserveResponse(0) {
				t.Fatal("same context acquired a second charge")
			}
			next := nagiHTTPResponseContext{limits: context.limits, pool: pool}
			if next.reserveResponse(0) {
				t.Fatal("another request bypassed the occupied response slot")
			}
			context.releaseResponse()
			context.releaseResponse()
			if context.charge != 0 || pool.live != 0 || pool.retained != 0 || !next.reserveResponse(0) {
				t.Fatal("released response could not be readmitted")
			}
			if next.charge != 64 || pool.live != 1 || pool.retained != 64 {
				t.Fatal("empty body did not reserve metadata and one slot")
			}
			next.releaseResponse()
		})
	}
}

type nagiHTTPReservationRecorder struct {
	*httptest.ResponseRecorder
	beforeWriteHeader func()
}

func (recorder nagiHTTPReservationRecorder) WriteHeader(status int) {
	recorder.beforeWriteHeader()
	recorder.ResponseRecorder.WriteHeader(status)
}

func TestNagiHTTPResponseReservationDuringDispatch(t *testing.T) {
	options := nagiHTTPDefaultOptions()
	options.wire = map[string]struct{}{"localhost": {}}
	options.authorityBytes = 64
	options.responses = &nagiHTTPResponseLimits{
		bodyBytes: 3, managedBytes: 128, managedCount: 4, serializationWrites: 1, live: 1, retained: 67,
		fallbackLive: 1, fallbackRetained: 256, fallbackManagedBytes: 256, fallbackManagedCount: 5,
	}
	app := nagiHTTPAppDefault(nagiUnit{})
	var context *nagiHTTPResponseContext
	var panics bool
	app.routes[nagiHTTPRouteKey{nagiHTTPGet, "/"}] = &nagiHTTPRouteEntry[nagiUnit]{
		handler: func(request nagiHTTPRequest, _ *nagiUnit, _ nagiUnit) *nagiFuture[nagiResult[nagiHTTPResponse, nagiError]] {
			return nagiCold(func() nagiResult[nagiHTTPResponse, nagiError] {
				context = nagiHTTPContext(request)
				result := nagiHTTPText(context, nagiHTTPOK, "abc")
				if !result.ok {
					t.Fatalf("ordinary response was not admitted: %+v", result)
				}
				if panics {
					panic("fixture handler failed before committing its response")
				}
				return result
			})
		},
		bucket: nagiHTTPBucket{policy: nagiHTTPRatePolicy{burst: 2, refill: time.Second}},
	}
	handler := nagiHTTPDispatch(app, options)
	for _, tc := range []struct {
		panics bool
		status int
		body   string
	}{
		{true, http.StatusInternalServerError, http.StatusText(http.StatusInternalServerError)},
		{false, http.StatusOK, "abc"},
	} {
		panics = tc.panics
		recorder := nagiHTTPReservationRecorder{httptest.NewRecorder(), func() {
			if context == nil || context.charge != 67 || context.pool.live != 1 {
				t.Fatal("ordinary response charge was released before headers")
			}
		}}
		request := httptest.NewRequest(http.MethodGet, "/", nil)
		request.Host = "localhost"
		handler.ServeHTTP(recorder, request)
		if recorder.Code != tc.status || recorder.Body.String() != tc.body {
			t.Fatalf("got status %d and body %q", recorder.Code, recorder.Body.String())
		}
		if context.charge != 0 || context.pool.live != 0 || context.pool.retained != 0 {
			t.Fatal("dispatch did not release the response charge")
		}
	}
}

func TestNagiHTTPResponseReservationWriterBounds(t *testing.T) {
	buf := []byte("___")
	writer := nagiHTTPFiniteWriter{buffer: buf, writesLeft: 3}
	if n, err := writer.Write([]byte("ab")); n != 2 || err != nil {
		t.Fatalf("first write = %d, %v", n, err)
	}
	if n, err := writer.WriteString("cd"); n != 0 || err != errNagiHTTPBodyLimit {
		t.Fatalf("oversized write = %d, %v", n, err)
	}
	if n, err := writer.WriteString("c"); n != 0 || err != errNagiHTTPBodyLimit {
		t.Fatalf("write after failure = %d, %v", n, err)
	}
	if string(buf) != "ab_" || writer.used != 2 || writer.writesLeft != 1 {
		t.Fatal("failed writes changed the payload, position or sticky accounting")
	}

	writer = nagiHTTPFiniteWriter{buffer: make([]byte, 1), writesLeft: 2}
	if n, err := writer.Write(nil); n != 0 || err != nil {
		t.Fatalf("empty write = %d, %v", n, err)
	}
	if n, err := writer.WriteString("a"); n != 1 || err != nil || string(writer.buffer) != "a" {
		t.Fatalf("exact bytes and writes = %d, %v", n, err)
	}
	if n, err := writer.WriteString("b"); n != 0 || err != errNagiHTTPWorkLimit {
		t.Fatalf("exhausted work and body = %d, %v", n, err)
	}
	if n, err := writer.Write(nil); n != 0 || err != errNagiHTTPWorkLimit {
		t.Fatalf("empty write after work failure = %d, %v", n, err)
	}
}

func TestNagiHTTPResponseReservationWriterFailureReleases(t *testing.T) {
	const media = "text/plain; charset=utf-8"
	limits := nagiHTTPResponseBudget(3, 128, 4, 128, 4, 1, 1, 67, 1, 256, 256, 5)
	if !limits.ok {
		t.Fatalf("response limits: %+v", limits)
	}
	pool := &nagiHTTPResponsePool{}
	context := nagiHTTPResponseContext{limits: limits.value, pool: pool}
	for _, tc := range []struct {
		name    string
		fill    func(*nagiHTTPFiniteWriter) error
		kind    uint8
		message string
	}{
		{"body", func(w *nagiHTTPFiniteWriter) error { _, _ = w.WriteString("abcd"); return errors.New("fixture detail") }, nagiInvalid, "response body limit"},
		{"work", func(w *nagiHTTPFiniteWriter) error { _, _ = w.Write(nil); _, _ = w.WriteString("abc"); return nil }, nagiInvalid, "response serialization limit"},
		{"serializer", func(w *nagiHTTPFiniteWriter) error {
			_, _ = w.WriteString("ab")
			return errors.New("fixture detail must not appear in the response")
		}, nagiInternal, "response serialization failed"},
		{"underfill", func(w *nagiHTTPFiniteWriter) error { _, _ = w.WriteString("ab"); return nil }, nagiInternal, "response body size mismatch"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			result := nagiHTTPProduce(&context, nagiHTTPOK, media, 3, tc.fill)
			if result.ok || result.problem.kind != tc.kind || result.problem.message != tc.message || result.value.body != nil {
				t.Fatalf("failed writer published a response or returned the wrong error: %+v", result)
			}
			if context.charge != 0 || pool.live != 0 || pool.retained != 0 {
				t.Fatal("failed writer did not release the reservation")
			}
			retry := nagiHTTPBytes(&context, nagiHTTPOK, []byte("abc"))
			if !retry.ok || string(retry.value.body) != "abc" || context.charge != 67 {
				t.Fatalf("failed writer prevented same-context retry: %+v", retry)
			}
			context.releaseResponse()
		})
	}
}

// Only the internal producer is used here. The public http.json builder stays
// gated, and the caller supplies the known size instead of measuring JSON.
func TestNagiHTTPResponseReservationScalarSink(t *testing.T) {
	for _, tc := range []struct {
		name   string
		size   int
		writes int64
		want   error
	}{
		{"exact", 5, 4, nil},
		{"body", 4, 4, errNagiHTTPBodyLimit},
		{"writes", 5, 3, errNagiHTTPWorkLimit},
	} {
		t.Run(tc.name, func(t *testing.T) {
			limits := nagiHTTPResponseBudget(int64(tc.size), 128, 4, 128, 4, tc.writes, 1, 69, 1, 256, 256, 5)
			if !limits.ok {
				t.Fatalf("response limits: %+v", limits)
			}
			pool := &nagiHTTPResponsePool{}
			context := nagiHTTPResponseContext{limits: limits.value, pool: pool}
			produced := nagiHTTPProduce(&context, nagiHTTPOK, "application/json", tc.size, func(writer *nagiHTTPFiniteWriter) error {
				if context.charge != int64(tc.size)+64 || pool.live != 1 {
					t.Fatal("serializer ran without a reservation")
				}
				err := nagiJSONWriteScalar(writer.WriteString, "a\n")
				if err != tc.want {
					t.Fatalf("serializer error = %v, want %v", err, tc.want)
				}
				return err
			})
			if tc.want == nil {
				if !produced.ok || produced.value.media != "application/json" || string(produced.value.body) != `"a\n"` {
					t.Fatalf("unexpected scalar response: %+v", produced)
				}
				context.releaseResponse()
				return
			}
			if produced.ok || produced.value.body != nil || produced.problem.kind != nagiInvalid ||
				produced.problem.message != tc.want.Error() || context.charge != 0 || pool.retained != 0 || pool.live != 0 {
				t.Fatalf("failure retained data or quota: %+v", produced)
			}
			retry := nagiHTTPText(&context, nagiHTTPOK, "ok")
			if !retry.ok {
				t.Fatalf("response slot was not reusable: %+v", retry)
			}
			context.releaseResponse()
		})
	}
}

// The public JSON constructor is gated. This probe supplies the managed
// fields that Rust's JSON finalizer would add, then measures Go's existing
// preflight and writer without claiming Go installs those fields itself.
func TestNagiHTTPResponseReservationJSONHeaderBudget(t *testing.T) {
	const media = "application/json"
	const body = `"a\n"`
	const csp = "default-src 'none'; script-src 'none'; style-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
	const managedBytes = int64(len("Content-Type: " + media + "\r\nContent-Length: 5\r\nConnection: close\r\n" +
		"Content-Security-Policy: " + csp + "\r\nX-Content-Type-Options: nosniff\r\nVary: Origin\r\n"))
	for _, tc := range []struct {
		name         string
		bytes, count int64
		wantOK       bool
	}{
		{"exact", managedBytes, 6, true},
		{"one byte short", managedBytes - 1, 6, false},
		{"one field short", managedBytes, 5, false},
	} {
		t.Run(tc.name, func(t *testing.T) {
			pool := &nagiHTTPResponsePool{}
			recorder := httptest.NewRecorder()
			headers := recorder.Header()
			headers.Set("Content-Security-Policy", csp)
			headers.Set("X-Content-Type-Options", "nosniff")
			headers.Set("Vary", "Origin")
			context := nagiHTTPResponseContext{
				headers: headers, pool: pool,
				limits: nagiHTTPResponseLimits{
					bodyBytes: 5, serializationWrites: 4, live: 1, retained: 69,
					managedBytes: tc.bytes, managedCount: tc.count,
				},
			}
			called := false
			produced := nagiHTTPProduce(&context, nagiHTTPOK, media, len(body), func(w *nagiHTTPFiniteWriter) error {
				called = true
				return nagiJSONWriteScalar(w.WriteString, "a\n")
			})
			if !tc.wantOK {
				if produced.ok || produced.problem.kind != nagiInvalid || called ||
					context.charge != 0 || pool.live != 0 || pool.retained != 0 || recorder.Body.Len() != 0 {
					t.Fatalf("budget refusal serialized or retained a response: %+v", produced)
				}
				return
			}
			if !produced.ok || !called || context.charge != 69 {
				t.Fatalf("exact budget refused or lost its charge: %+v", produced)
			}
			nagiHTTPWrite(recorder, int(produced.value.status), produced.value.media, produced.value.body)
			response := recorder.Result()
			defer response.Body.Close()
			if response.StatusCode != 200 || recorder.Body.String() != body ||
				response.Header.Get("Content-Type") != media || response.Header.Get("Content-Length") != "5" ||
				response.Header.Get("Content-Security-Policy") != csp ||
				response.Header.Get("X-Content-Type-Options") != "nosniff" || response.Header.Get("Vary") != "Origin" {
				t.Fatalf("response = %d %v %q", response.StatusCode, response.Header, recorder.Body.String())
			}
			context.releaseResponse()
			if pool.live != 0 || pool.retained != 0 {
				t.Fatal("response charge survived release")
			}
		})
	}
}

func TestNagiHTTPResponseReservationKnownSizeDoesNotProveMaximum(t *testing.T) {
	pool := &nagiHTTPResponsePool{}
	context := nagiHTTPResponseContext{
		pool:   pool,
		limits: nagiHTTPResponseLimits{bodyBytes: 8, serializationWrites: 4, live: 2, retained: 71, managedBytes: 128, managedCount: 4},
	}
	if context.reserveResponse(context.limits.bodyBytes) || context.charge != 0 || pool.live != 0 {
		t.Fatal("full body ceiling was admitted with a one-byte short retained budget")
	}
	known := nagiHTTPText(&context, nagiHTTPOK, "short")
	if !known.ok || context.charge != 69 {
		t.Fatalf("known five-byte body should be admitted: %+v", known)
	}
	context.releaseResponse()

	context.limits.retained++
	if !context.reserveResponse(context.limits.bodyBytes) || context.charge != 72 || pool.retained != 72 {
		t.Fatal("exact body ceiling was not reserved")
	}
	context.releaseResponse()
	if context.charge != 0 || pool.live != 0 || pool.retained != 0 {
		t.Fatal("ceiling reservation was not returned")
	}
}
