package main

import (
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
			filled := false
			result := nagiHTTPProduce(&context, nagiHTTPOK, media, 3, func(out []byte) {
				filled = true
				if context.charge != 67 || pool.live != 1 || pool.retained != 67 {
					t.Fatal("body fill ran before the response was charged")
				}
				copy(out, "abc")
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
		bodyBytes: 3, managedBytes: 128, managedCount: 4, live: 1, retained: 67,
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
