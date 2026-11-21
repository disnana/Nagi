package main

import (
	"context"
	"errors"
	"io"
	"net"
	"net/http"
	"net/netip"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"sync"
	"time"
)

type nagiHTTPMethod string
type nagiHTTPStatus int

const (
	nagiHTTPGet  nagiHTTPMethod = http.MethodGet
	nagiHTTPPost nagiHTTPMethod = http.MethodPost
	nagiHTTPOK   nagiHTTPStatus = http.StatusOK
)

type nagiHTTPRequest struct {
	body    []byte
	context *nagiHTTPResponseContext
}

type nagiHTTPResponse struct {
	status nagiHTTPStatus
	media  string
	body   []byte
}

type nagiHTTPResponseLimits struct {
	bodyBytes, managedBytes, managedCount                                      int64
	live, retained                                                             int64
	fallbackLive, fallbackRetained, fallbackManagedBytes, fallbackManagedCount int64
}

type nagiHTTPResponsePool struct {
	mu             sync.Mutex
	live, retained int64
}

func (pool *nagiHTTPResponsePool) acquire(bytes, maxLive, maxRetained int64) bool {
	pool.mu.Lock()
	defer pool.mu.Unlock()
	if pool.live == maxLive || bytes > maxRetained-pool.retained {
		return false
	}
	pool.live++
	pool.retained += bytes
	return true
}

func (pool *nagiHTTPResponsePool) release(bytes int64) {
	pool.mu.Lock()
	defer pool.mu.Unlock()
	pool.live--
	pool.retained -= bytes
}

type nagiHTTPResponseContext struct {
	headers http.Header
	limits  nagiHTTPResponseLimits
	pool    *nagiHTTPResponsePool
	charge  int64
}

// reserveResponse holds the request's ordinary response charge until dispatch
// returns, including the time spent writing a fallback response.
func (context *nagiHTTPResponseContext) reserveResponse(bodyBytes int64) bool {
	const responseMetadata = 64
	limits := context.limits
	if context.charge != 0 || bodyBytes > limits.retained-responseMetadata ||
		!context.pool.acquire(bodyBytes+responseMetadata, limits.live, limits.retained) {
		return false
	}
	context.charge = bodyBytes + responseMetadata
	return true
}

func (context *nagiHTTPResponseContext) releaseResponse() {
	if context.charge != 0 {
		context.pool.release(context.charge)
		context.charge = 0
	}
}

func nagiHTTPContext(request nagiHTTPRequest) *nagiHTTPResponseContext {
	return request.context
}

func nagiHTTPFail[T any](message string) nagiResult[T, nagiError] {
	return nagiErr[T, nagiError](nagiError{message: message, kind: nagiInvalid})
}

func nagiHTTPProduce(context *nagiHTTPResponseContext, status nagiHTTPStatus, media string, size int, fill func([]byte)) nagiResult[nagiHTTPResponse, nagiError] {
	limits := context.limits
	bodyBytes := int64(size)
	headerBytes := int64(len("Content-Type: \r\nContent-Length: \r\nConnection: close\r\n") + len(media) + len(strconv.Itoa(size)))
	extraBytes, extraCount := nagiHTTPExtraHeaders(context.headers)
	if bodyBytes > limits.bodyBytes || headerBytes+extraBytes > limits.managedBytes || 3+extraCount > limits.managedCount {
		return nagiHTTPFail[nagiHTTPResponse]("HTTP response size limit exceeded")
	}
	if !context.reserveResponse(bodyBytes) {
		return nagiErr[nagiHTTPResponse, nagiError](nagiError{message: "HTTP response budget exhausted", kind: nagiBusy})
	}
	body := make([]byte, size)
	fill(body)
	return nagiOk[nagiHTTPResponse, nagiError](nagiHTTPResponse{status: status, media: media, body: body})
}

func nagiHTTPBytes(context *nagiHTTPResponseContext, status nagiHTTPStatus, body []byte) nagiResult[nagiHTTPResponse, nagiError] {
	return nagiHTTPProduce(context, status, "application/octet-stream", len(body), func(out []byte) { copy(out, body) })
}

func nagiHTTPText(context *nagiHTTPResponseContext, status nagiHTTPStatus, body string) nagiResult[nagiHTTPResponse, nagiError] {
	return nagiHTTPProduce(context, status, "text/plain; charset=utf-8", len(body), func(out []byte) { copy(out, body) })
}

type nagiHTTPRateLimits struct {
	rules, retained int64
}

type nagiHTTPOptions struct {
	bodyBytes                           int64
	bodyTime, handlerTime, shutdownTime time.Duration
	connections, requests               int
	wire                                map[string]struct{}
	authorityBytes                      int64
	responses                           *nagiHTTPResponseLimits
	rates                               *nagiHTTPRateLimits
}

func nagiHTTPDefaultOptions() nagiHTTPOptions {
	return nagiHTTPOptions{
		bodyBytes: 1024 * 1024, bodyTime: 10 * time.Second,
		handlerTime: 2 * time.Second, shutdownTime: 10 * time.Second,
		connections: 1024, requests: 256,
	}
}

func nagiHTTPPositive(values ...int64) bool {
	for _, value := range values {
		if value <= 0 || int64(int(value)) != value {
			return false
		}
	}
	return true
}

func nagiHTTPMilliseconds(value int64) (time.Duration, bool) {
	if value <= 0 || value > int64(1<<63-1)/int64(time.Millisecond) {
		return 0, false
	}
	return time.Duration(value) * time.Millisecond, true
}

func nagiHTTPOptionsWithBody(bytes, bodyMS, handlerMS, shutdownMS int64) nagiResult[nagiHTTPOptions, nagiError] {
	bodyTime, bodyOK := nagiHTTPMilliseconds(bodyMS)
	handlerTime, handlerOK := nagiHTTPMilliseconds(handlerMS)
	shutdownTime, shutdownOK := nagiHTTPMilliseconds(shutdownMS)
	if !nagiHTTPPositive(bytes) || !bodyOK || !handlerOK || !shutdownOK {
		return nagiHTTPFail[nagiHTTPOptions]("invalid HTTP body limit or deadline")
	}
	options := nagiHTTPDefaultOptions()
	options.bodyBytes, options.bodyTime = bytes, bodyTime
	options.handlerTime, options.shutdownTime = handlerTime, shutdownTime
	return nagiOk[nagiHTTPOptions, nagiError](options)
}

func nagiHTTPCapacity(options nagiHTTPOptions, connections, requests int64) nagiResult[nagiHTTPOptions, nagiError] {
	if !nagiHTTPPositive(connections, requests) || connections > int64(^uint(0)>>4) || requests > int64(^uint(0)>>4) {
		return nagiHTTPFail[nagiHTTPOptions]("invalid HTTP capacity")
	}
	options.connections, options.requests = int(connections), int(requests)
	return nagiOk[nagiHTTPOptions, nagiError](options)
}

func nagiHTTPResponseBudget(body, userBytes, userCount, managedBytes, managedCount, writes, live, retained, fallbackLive, fallbackRetained, fallbackBytes, fallbackCount int64) nagiResult[nagiHTTPResponseLimits, nagiError] {
	if !nagiHTTPPositive(body, userBytes, userCount, managedBytes, managedCount, writes, live, retained, fallbackLive, fallbackRetained, fallbackBytes, fallbackCount) {
		return nagiHTTPFail[nagiHTTPResponseLimits]("invalid HTTP response limits")
	}
	return nagiOk[nagiHTTPResponseLimits, nagiError](nagiHTTPResponseLimits{
		bodyBytes: body, managedBytes: managedBytes, managedCount: managedCount,
		live: live, retained: retained, fallbackLive: fallbackLive, fallbackRetained: fallbackRetained,
		fallbackManagedBytes: fallbackBytes, fallbackManagedCount: fallbackCount,
	})
}

func nagiHTTPWithResponseLimits(options nagiHTTPOptions, limits nagiHTTPResponseLimits) nagiResult[nagiHTTPOptions, nagiError] {
	if limits.retained < 64 || limits.managedBytes < 64 || limits.managedCount < 3 ||
		limits.fallbackRetained < 128 || limits.fallbackManagedBytes < 128 || limits.fallbackManagedCount < 5 {
		return nagiHTTPFail[nagiHTTPOptions]("HTTP response limits cannot fit mandatory fields")
	}
	options.responses = &limits
	return nagiOk[nagiHTTPOptions, nagiError](options)
}

func nagiHTTPRateBudget(rules, domains, subjectSlots, retained, work int64) nagiResult[nagiHTTPRateLimits, nagiError] {
	// Public route charging is one table lookup and one bucket update. Subject
	// storage is never created for this backend's public routes.
	if !nagiHTTPPositive(rules, domains, subjectSlots, retained, work) || domains > 65536 {
		return nagiHTTPFail[nagiHTTPRateLimits]("invalid HTTP rate limits")
	}
	return nagiOk[nagiHTTPRateLimits, nagiError](nagiHTTPRateLimits{rules: rules, retained: retained})
}

func nagiHTTPWithRateLimits(options nagiHTTPOptions, limits nagiHTTPRateLimits) nagiResult[nagiHTTPOptions, nagiError] {
	options.rates = &limits
	return nagiOk[nagiHTTPOptions, nagiError](options)
}

func nagiHTTPCanonicalAuthority(value string) (string, bool) {
	host, port := value, ""
	if strings.HasPrefix(value, "[") {
		end := strings.IndexByte(value, ']')
		if end < 0 {
			return "", false
		}
		ip, err := netip.ParseAddr(value[1:end])
		if err != nil || !ip.Is6() || ip.Zone() != "" {
			return "", false
		}
		host, port = "["+ip.String()+"]", value[end+1:]
	} else {
		if end := strings.IndexByte(value, ':'); end >= 0 {
			host, port = value[:end], value[end:]
		}
		if len(host) > 253 {
			return "", false
		}
		numeric := true
		for _, label := range strings.Split(host, ".") {
			if len(label) == 0 || len(label) > 63 || label[0] == '-' || label[len(label)-1] == '-' {
				return "", false
			}
			for _, c := range label {
				digit := c >= '0' && c <= '9'
				if !digit && !(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c == '-') {
					return "", false
				}
				numeric = numeric && digit
			}
		}
		if numeric {
			ip, err := netip.ParseAddr(host)
			if err != nil || !ip.Is4() {
				return "", false
			}
		}
		host = strings.ToLower(host)
	}
	if port != "" {
		if port[0] != ':' {
			return "", false
		}
		number, err := strconv.ParseUint(port[1:], 10, 16)
		if err != nil || number == 0 || strconv.FormatUint(number, 10) != port[1:] {
			return "", false
		}
	}
	return host + port, true
}

func nagiHTTPAuthority(options nagiHTTPOptions, origin string, values []string, entries, bytes int64) nagiResult[nagiHTTPOptions, nagiError] {
	if options.wire != nil || !nagiHTTPPositive(entries, bytes) || len(values) == 0 ||
		int64(len(values)) > entries || int64(len(origin)) > bytes ||
		len(origin) < len("https://") || !strings.EqualFold(origin[:len("https://")], "https://") {
		return nagiHTTPFail[nagiHTTPOptions]("invalid HTTP authority configuration")
	}
	if _, ok := nagiHTTPCanonicalAuthority(origin[len("https://"):]); !ok {
		return nagiHTTPFail[nagiHTTPOptions]("invalid HTTP external origin")
	}
	wire := make(map[string]struct{}, len(values))
	remaining := bytes
	for _, value := range values {
		remaining -= int64(len(value))
		canonical, ok := nagiHTTPCanonicalAuthority(value)
		_, duplicate := wire[canonical]
		if remaining < 0 || !ok || duplicate {
			return nagiHTTPFail[nagiHTTPOptions]("invalid or duplicate HTTP wire authority")
		}
		wire[canonical] = struct{}{}
	}
	options.wire, options.authorityBytes = wire, bytes
	return nagiOk[nagiHTTPOptions, nagiError](options)
}

type nagiHTTPRatePolicy struct {
	burst  int64
	refill time.Duration
}

func nagiHTTPMakeRatePolicy(burst, refillMS int64) nagiResult[nagiHTTPRatePolicy, nagiError] {
	refill, ok := nagiHTTPMilliseconds(refillMS)
	if !ok || burst < 1 || burst > 1<<32-1 {
		return nagiHTTPFail[nagiHTTPRatePolicy]("invalid HTTP rate policy")
	}
	return nagiOk[nagiHTTPRatePolicy, nagiError](nagiHTTPRatePolicy{burst: burst, refill: refill})
}

type nagiHTTPBucket struct {
	policy nagiHTTPRatePolicy
	mu     sync.Mutex
	tokens int64
	last   time.Time
}

func (bucket *nagiHTTPBucket) charge() time.Duration {
	bucket.mu.Lock()
	defer bucket.mu.Unlock()
	now := time.Now()
	if bucket.last.IsZero() {
		bucket.last, bucket.tokens = now, bucket.policy.burst
	}
	elapsed := now.Sub(bucket.last)
	if periods := elapsed / bucket.policy.refill; periods > 0 {
		bucket.tokens = min(bucket.policy.burst, bucket.tokens+int64(periods))
		bucket.last = now.Add(-(elapsed % bucket.policy.refill))
	}
	if bucket.tokens == 0 {
		return bucket.policy.refill - now.Sub(bucket.last)
	}
	bucket.tokens--
	return 0
}

type nagiHTTPPolicy[S any] struct{}
type nagiHTTPRouteKey struct {
	method nagiHTTPMethod
	path   string
}

type nagiHTTPRouteEntry[S any] struct {
	handler func(nagiHTTPRequest, *S, nagiUnit) *nagiFuture[nagiResult[nagiHTTPResponse, nagiError]]
	bucket  nagiHTTPBucket
	cors    *nagiHTTPCORSPolicy
}

type nagiHTTPApp[S any] struct {
	state  *S
	routes map[nagiHTTPRouteKey]*nagiHTTPRouteEntry[S]
}

func nagiHTTPPublicPolicy[S any]() nagiHTTPPolicy[S] { return nagiHTTPPolicy[S]{} }

func nagiHTTPAppDefault[S any](state S) nagiHTTPApp[S] {
	return nagiHTTPApp[S]{state: &state, routes: make(map[nagiHTTPRouteKey]*nagiHTTPRouteEntry[S])}
}

func nagiHTTPRoute[S any](app nagiHTTPApp[S], method nagiHTTPMethod, path string, _ nagiHTTPPolicy[S], handler func(nagiHTTPRequest, *S, nagiUnit) *nagiFuture[nagiResult[nagiHTTPResponse, nagiError]]) nagiResult[nagiHTTPApp[S], nagiError] {
	if !strings.HasPrefix(path, "/") || strings.ContainsAny(path, "{}?#\r\n") {
		return nagiHTTPFail[nagiHTTPApp[S]]("Go HTTP routing requires an exact path")
	}
	key := nagiHTTPRouteKey{method, path}
	if _, exists := app.routes[key]; exists {
		return nagiHTTPFail[nagiHTTPApp[S]]("duplicate HTTP route")
	}
	app.routes[key] = &nagiHTTPRouteEntry[S]{handler: handler}
	return nagiOk[nagiHTTPApp[S], nagiError](app)
}

func nagiHTTPRateRoute[S any](app nagiHTTPApp[S], method nagiHTTPMethod, path string, policy nagiHTTPRatePolicy) nagiResult[nagiHTTPApp[S], nagiError] {
	route := app.routes[nagiHTTPRouteKey{method, path}]
	if route == nil || route.bucket.policy.refill != 0 {
		return nagiHTTPFail[nagiHTTPApp[S]]("HTTP rate attachment requires an unconfigured exact route")
	}
	route.bucket.policy = policy
	return nagiOk[nagiHTTPApp[S], nagiError](app)
}

type nagiHTTPConn struct {
	net.Conn
	release func()
	once    sync.Once
}

func (conn *nagiHTTPConn) Close() error {
	err := conn.Conn.Close()
	conn.once.Do(conn.release)
	return err
}

type nagiHTTPListener struct {
	net.Listener
	slots chan struct{}
	done  chan struct{}
	once  sync.Once
}

func (listener *nagiHTTPListener) Accept() (net.Conn, error) {
	select {
	case listener.slots <- struct{}{}:
	case <-listener.done:
		return nil, net.ErrClosed
	}
	release := func() { <-listener.slots }
	conn, err := listener.Listener.Accept()
	if err != nil {
		release()
		return nil, err
	}
	return &nagiHTTPConn{Conn: conn, release: release}, nil
}

func (listener *nagiHTTPListener) Close() error {
	listener.once.Do(func() { close(listener.done) })
	return listener.Listener.Close()
}

func nagiHTTPHeadStatus(request *http.Request, options nagiHTTPOptions) int {
	if request.ProtoMajor == 1 && request.ProtoMinor == 0 {
		return http.StatusHTTPVersionNotSupported
	}
	host, valid := nagiHTTPCanonicalAuthority(request.Host)
	_, allowed := options.wire[host]
	if !valid || !allowed || int64(len(request.Host)) > options.authorityBytes ||
		request.Proto != "HTTP/1.1" || request.URL.IsAbs() || !strings.HasPrefix(request.RequestURI, "/") {
		return http.StatusBadRequest
	}
	fields := 1 // Host, represented separately by net/http.
	bytes := len("Host: \r\n") + len(request.Host)
	for name, values := range request.Header {
		lower := strings.ToLower(name)
		if lower == "forwarded" || strings.HasPrefix(lower, "x-forwarded-") {
			return http.StatusBadRequest
		}
		fields += len(values)
		for _, value := range values {
			bytes += len(name) + len(value) + len(": \r\n")
		}
	}
	if fields > 100 || bytes > 32*1024 {
		return http.StatusRequestHeaderFieldsTooLarge
	}
	return 0
}

func nagiHTTPWrite(writer http.ResponseWriter, status int, media string, body []byte) {
	writer.Header()["Date"] = nil
	writer.Header().Set("Content-Type", media)
	writer.Header().Set("Content-Length", strconv.Itoa(len(body)))
	writer.WriteHeader(status)
	_, _ = writer.Write(body)
}

func nagiHTTPFallback(writer http.ResponseWriter, pool *nagiHTTPResponsePool, limits nagiHTTPResponseLimits, status int, closeConnection bool) {
	const metadata = 64
	message := http.StatusText(status)
	if status == http.StatusRequestEntityTooLarge {
		message = "Payload Too Large"
	}
	charge := int64(metadata + len(message))
	writer.Header().Set("Cache-Control", "no-store")
	const media = "text/plain; charset=utf-8"
	headerBytes := len("Content-Type: \r\nContent-Length: \r\nConnection: close\r\n") + len(media) + len(strconv.Itoa(len(message)))
	extraBytes, extraCount := nagiHTTPExtraHeaders(writer.Header())
	if int64(headerBytes)+extraBytes > limits.fallbackManagedBytes || 3+extraCount > limits.fallbackManagedCount ||
		!pool.acquire(charge, limits.fallbackLive, limits.fallbackRetained) {
		clear(writer.Header())
		writer.Header().Set("Cache-Control", "no-store")
		writer.Header().Set("Connection", "close")
		writer.Header().Set("Content-Length", "0")
		writer.WriteHeader(http.StatusServiceUnavailable)
		return
	}
	defer pool.release(charge)
	if closeConnection {
		writer.Header().Set("Connection", "close")
	}
	nagiHTTPWrite(writer, status, media, []byte(message))
}

func nagiHTTPRead(request *http.Request, control *http.ResponseController, options nagiHTTPOptions) ([]byte, int) {
	if request.ContentLength < 0 {
		// This slice accepts fixed-length requests and requests with no body.
		return nil, http.StatusNotImplemented
	}
	if request.ContentLength == 0 {
		return nil, 0
	}
	if control.SetReadDeadline(time.Now().Add(options.bodyTime)) != nil {
		return nil, http.StatusInternalServerError
	}
	defer control.SetReadDeadline(time.Time{})
	body, err := io.ReadAll(io.LimitReader(request.Body, request.ContentLength))
	if err != nil {
		var timeout net.Error
		if errors.As(err, &timeout) && timeout.Timeout() {
			return nil, http.StatusRequestTimeout
		}
		return nil, http.StatusBadRequest
	}
	if int64(len(body)) != request.ContentLength {
		return nil, http.StatusBadRequest
	}
	return body, 0
}

func nagiHTTPDispatch[S any](app nagiHTTPApp[S], options nagiHTTPOptions) http.Handler {
	requests := make(chan struct{}, options.requests)
	pool, fallback := &nagiHTTPResponsePool{}, &nagiHTTPResponsePool{}
	limits := *options.responses
	return http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		control := http.NewResponseController(writer)
		refuse := func(status int, closeConnection bool) {
			now := time.Now()
			if closeConnection {
				// A final refusal must not wait for an unread body after sending
				// its response, even when the peer waits for 100 Continue.
				_ = control.SetReadDeadline(now)
			}
			_ = control.SetWriteDeadline(now.Add(10 * time.Second))
			nagiHTTPFallback(writer, fallback, limits, status, closeConnection)
		}
		headers := writer.Header()
		preflight := request.Method == http.MethodOptions &&
			(len(request.Header.Values(nagiCORSRequestMethod)) != 0 || len(request.Header.Values(nagiCORSRequestHeaders)) != 0)
		if preflight {
			headers.Set("Vary", nagiCORSPreflightVary)
		}
		if status := nagiHTTPHeadStatus(request, options); status != 0 {
			refuse(status, true)
			return
		}
		path := request.URL.EscapedPath()
		get := app.routes[nagiHTTPRouteKey{nagiHTTPGet, path}]
		post := app.routes[nagiHTTPRouteKey{nagiHTTPPost, path}]
		var route *nagiHTTPRouteEntry[S]
		method := request.Method
		if preflight {
			var ok bool
			method, ok = nagiCORSPreflightMethod(request.Header)
			if !ok {
				refuse(http.StatusBadRequest, true)
				return
			}
		}
		switch method {
		case http.MethodGet, http.MethodHead:
			route = get
		case http.MethodPost:
			route = post
		}
		if route == nil {
			switch {
			case get != nil && post != nil:
				writer.Header().Set("Allow", "GET, HEAD, POST")
			case get != nil:
				writer.Header().Set("Allow", "GET, HEAD")
			case post != nil:
				writer.Header().Set("Allow", "POST")
			default:
				refuse(http.StatusNotFound, preflight)
				return
			}
			refuse(http.StatusMethodNotAllowed, preflight)
			return
		}
		if route.cors != nil {
			if !preflight {
				headers.Set("Vary", "Origin")
			}
			if origin, status := nagiCORSRequestOrigin(request.Header, route.cors.originBytes); status == 0 {
				route.cors.grant(headers, origin)
			}
		}
		select {
		case requests <- struct{}{}:
			defer func() { <-requests }()
		default:
			refuse(http.StatusServiceUnavailable, true)
			return
		}
		if request.ContentLength > options.bodyBytes {
			refuse(http.StatusRequestEntityTooLarge, true)
			return
		}
		if preflight {
			if status := nagiCORSPreflight(request, route.cors, method, headers); status != 0 {
				refuse(status, true)
				return
			}
			bytes, count := nagiHTTPExtraHeaders(headers)
			const metadata = 64
			if bytes+int64(len("Connection: close\r\n")) > limits.managedBytes || count+1 > limits.managedCount ||
				!pool.acquire(metadata, limits.live, limits.retained) {
				clear(headers)
				headers.Set("Vary", nagiCORSPreflightVary)
				refuse(http.StatusServiceUnavailable, true)
				return
			}
			defer pool.release(metadata)
			_ = control.SetWriteDeadline(time.Now().Add(10 * time.Second))
			headers["Date"] = nil
			writer.WriteHeader(http.StatusNoContent)
			return
		}
		if retry := route.bucket.charge(); retry > 0 {
			writer.Header().Set("Retry-After", strconv.FormatInt(int64((retry-1)/time.Second+1), 10))
			refuse(http.StatusTooManyRequests, false)
			return
		}
		body, status := nagiHTTPRead(request, control, options)
		if status != 0 {
			refuse(status, true)
			return
		}
		responseContext := &nagiHTTPResponseContext{headers: headers, limits: limits, pool: pool}
		defer responseContext.releaseResponse()
		start := time.Now()
		result, panicked := nagiHTTPInvoke(route, nagiHTTPRequest{body: body, context: responseContext}, app.state)
		if time.Since(start) > options.handlerTime {
			refuse(http.StatusGatewayTimeout, true)
			return
		}
		if panicked {
			refuse(http.StatusInternalServerError, true)
			return
		}
		if !result.ok {
			var mapped nagiResult[nagiHTTPResponse, nagiError]
			switch result.problem.kind {
			case nagiInvalid:
				mapped = nagiHTTPText(responseContext, http.StatusBadRequest, result.problem.message)
			case nagiBusy:
				refuse(http.StatusServiceUnavailable, false)
				return
			default:
				refuse(http.StatusInternalServerError, false)
				return
			}
			if !mapped.ok {
				refuse(http.StatusInternalServerError, false)
				return
			}
			result = mapped
		}
		_ = control.SetWriteDeadline(time.Now().Add(10 * time.Second))
		response := result.value
		nagiHTTPWrite(writer, int(response.status), response.media, response.body)
	})
}

func nagiHTTPInvoke[S any](route *nagiHTTPRouteEntry[S], request nagiHTTPRequest, state *S) (result nagiResult[nagiHTTPResponse, nagiError], panicked bool) {
	defer func() {
		if recover() != nil {
			panicked = true
		}
	}()
	return route.handler(request, state, nagiUnit{}).await(), false
}

func nagiHTTPServe[S any](app nagiHTTPApp[S], port int64, options nagiHTTPOptions) *nagiFuture[nagiResult[nagiUnit, nagiError]] {
	return nagiCold(func() nagiResult[nagiUnit, nagiError] {
		if port <= 0 || port > 65535 {
			return nagiHTTPFail[nagiUnit]("HTTP port must be between 1 and 65535")
		}
		if options.wire == nil || options.responses == nil || options.rates == nil {
			return nagiHTTPFail[nagiUnit]("HTTP authority, response limits, and rate limits must be explicit")
		}
		routes := int64(len(app.routes))
		const rateBucketBytes = 64
		if routes > options.rates.rules || routes > options.rates.retained/rateBucketBytes {
			return nagiHTTPFail[nagiUnit]("HTTP rate catalogue exceeds configured limits")
		}
		responseLimits := *options.responses
		for _, route := range app.routes {
			if route.cors != nil {
				if route.cors.retained > responseLimits.retained-64 {
					return nagiHTTPFail[nagiUnit]("CORS metadata exceeds the HTTP response budget")
				}
				responseLimits.retained -= route.cors.retained
			}
			if route.bucket.policy.refill == 0 {
				return nagiHTTPFail[nagiUnit]("every HTTP route requires an explicit rate policy")
			}
		}
		options.responses = &responseLimits
		socket, err := net.Listen("tcp4", net.JoinHostPort("127.0.0.1", strconv.FormatInt(port, 10)))
		if err != nil {
			return nagiHTTPFail[nagiUnit](err.Error())
		}
		listener := &nagiHTTPListener{Listener: socket, slots: make(chan struct{}, options.connections), done: make(chan struct{})}
		server := &http.Server{
			Handler: nagiHTTPDispatch(app, options), ReadHeaderTimeout: 10 * time.Second,
			IdleTimeout: 10 * time.Second, MaxHeaderBytes: 32 * 1024,
		}
		interrupt, stop := signal.NotifyContext(context.Background(), os.Interrupt)
		defer stop()
		finished := make(chan struct{})
		go func() {
			defer close(finished)
			<-interrupt.Done()
			deadline, cancel := context.WithTimeout(context.Background(), options.shutdownTime)
			defer cancel()
			if server.Shutdown(deadline) != nil {
				_ = server.Close()
			}
		}()
		err = server.Serve(listener)
		stop()
		<-finished
		if !errors.Is(err, http.ErrServerClosed) {
			return nagiHTTPFail[nagiUnit](err.Error())
		}
		return nagiOk[nagiUnit, nagiError](nagiUnit{})
	})
}
