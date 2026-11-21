package main

import (
	"net/http"
	"slices"
	"sort"
	"strconv"
	"strings"
)

type nagiHTTPCORSCredentials bool

type nagiHTTPCORSPolicy struct {
	origins, headers                      []string
	credentials                           nagiHTTPCORSCredentials
	maxAge                                string
	originBytes, headerCount, headerBytes int
	retained                              int64
}

const (
	nagiCORSRequestMethod  = "Access-Control-Request-Method"
	nagiCORSRequestHeaders = "Access-Control-Request-Headers"
	nagiCORSPreflightVary  = "Origin, Access-Control-Request-Method, Access-Control-Request-Headers"
)

func nagiCORSOrigin(raw string) (string, bool) {
	scheme, value, found := strings.Cut(raw, "://")
	if !found || (scheme != "http" && scheme != "https") {
		return "", false
	}
	authority, ok := nagiHTTPCanonicalAuthority(value)
	if !ok {
		return "", false
	}
	port := ":80"
	if scheme == "https" {
		port = ":443"
	}
	return scheme + "://" + strings.TrimSuffix(authority, port), true
}

func nagiCORSToken(value string) bool {
	if value == "" || value == "*" {
		return false
	}
	for i := 0; i < len(value); i++ {
		b := value[i]
		if !(b >= 'a' && b <= 'z' || b >= 'A' && b <= 'Z' || b >= '0' && b <= '9' ||
			strings.IndexByte("!#$%&'*+-.^_`|~", b) >= 0) {
			return false
		}
	}
	return true
}

func nagiCORSHdrFits(values []string, count, bytes int, separator bool) bool {
	if len(values) > count {
		return false
	}
	for i, value := range values {
		if separator && i != 0 {
			bytes -= 2
		}
		if len(value) > bytes {
			return false
		}
		bytes -= len(value)
	}
	return true
}

func nagiHTTPCORSConfig(origins, headers []string, credentials nagiHTTPCORSCredentials, maxAge, maxOrigins, originBytes, maxHeaders, headerBytes int64) nagiResult[nagiHTTPCORSPolicy, nagiError] {
	if !nagiHTTPPositive(maxOrigins, originBytes, maxHeaders, headerBytes) || maxAge < 0 || maxAge > 1<<32-1 ||
		!nagiCORSHdrFits(origins, int(maxOrigins), int(originBytes), false) ||
		!nagiCORSHdrFits(headers, int(maxHeaders), int(headerBytes), true) {
		return nagiHTTPFail[nagiHTTPCORSPolicy]("invalid CORS limits")
	}
	policy := nagiHTTPCORSPolicy{
		origins: make([]string, 0, len(origins)), headers: make([]string, 0, len(headers)),
		credentials: credentials, maxAge: strconv.FormatInt(maxAge, 10), originBytes: int(originBytes),
		headerCount: int(maxHeaders), headerBytes: int(headerBytes), retained: 128,
	}
	for _, origin := range origins {
		canonical, ok := nagiCORSOrigin(origin)
		if !ok || slices.Contains(policy.origins, canonical) {
			return nagiHTTPFail[nagiHTTPCORSPolicy]("invalid or duplicate CORS origin")
		}
		policy.origins = append(policy.origins, canonical)
		policy.retained += 16 + int64(len(canonical))
	}
	if !nagiCORSHdrFits(policy.origins, int(maxOrigins), int(originBytes), false) {
		return nagiHTTPFail[nagiHTTPCORSPolicy]("canonical CORS origins exceed byte limit")
	}
	for _, header := range headers {
		name := strings.ToLower(header)
		if !nagiCORSToken(header) || slices.Contains(policy.headers, name) {
			return nagiHTTPFail[nagiHTTPCORSPolicy]("invalid or duplicate CORS header")
		}
		policy.headers = append(policy.headers, strings.Clone(name))
		policy.retained += 16 + int64(len(name))
	}
	return nagiOk[nagiHTTPCORSPolicy, nagiError](policy)
}

func nagiHTTPCORSRoute[S any](app nagiHTTPApp[S], method nagiHTTPMethod, path string, policy nagiHTTPCORSPolicy) nagiResult[nagiHTTPApp[S], nagiError] {
	route := app.routes[nagiHTTPRouteKey{method, path}]
	if route == nil || route.cors != nil || len(method) > policy.headerBytes {
		return nagiHTTPFail[nagiHTTPApp[S]]("CORS attachment requires an unconfigured exact route and supported method")
	}
	route.cors = &policy
	return nagiOk[nagiHTTPApp[S], nagiError](app)
}

func nagiCORSRequestOrigin(headers http.Header, limit int) (string, int) {
	values := headers.Values("Origin")
	if len(values) != 1 || len(values[0]) > limit {
		return "", http.StatusBadRequest
	}
	if values[0] == "null" {
		return "", http.StatusForbidden
	}
	origin, ok := nagiCORSOrigin(values[0])
	if !ok {
		return "", http.StatusBadRequest
	}
	return origin, 0
}

func (policy *nagiHTTPCORSPolicy) grant(headers http.Header, origin string) bool {
	index := slices.Index(policy.origins, origin)
	if index < 0 {
		return false
	}
	headers.Set("Access-Control-Allow-Origin", policy.origins[index])
	if policy.credentials {
		headers.Set("Access-Control-Allow-Credentials", "true")
	}
	return true
}

func nagiCORSPreflightMethod(headers http.Header) (string, bool) {
	methods := headers.Values(nagiCORSRequestMethod)
	if len(methods) != 1 || !nagiCORSToken(methods[0]) {
		return "", false
	}
	switch methods[0] {
	case "CONNECT", "TRACE", "TRACK":
		return "", false
	default:
		return methods[0], true
	}
}

func nagiCORSRequestedHeaders(headers http.Header, count, bytes int) ([]string, bool) {
	values := headers.Values(nagiCORSRequestHeaders)
	if len(values) == 0 {
		return nil, true
	}
	if len(values) != 1 || len(values[0]) > bytes {
		return nil, false
	}
	var names []string
	for _, piece := range strings.Split(values[0], ",") {
		token := strings.Trim(piece, " \t")
		name := strings.ToLower(token)
		if !nagiCORSToken(token) || len(names) >= count || slices.Contains(names, name) {
			return nil, false
		}
		names = append(names, name)
	}
	return names, nagiCORSHdrFits(names, count, bytes, true)
}

func nagiCORSPreflight(request *http.Request, policy *nagiHTTPCORSPolicy, method string, headers http.Header) int {
	if request.ContentLength != 0 || len(request.TransferEncoding) != 0 {
		return http.StatusBadRequest
	}
	for _, value := range request.Header.Values("Content-Length") {
		if value != "0" {
			return http.StatusBadRequest
		}
	}
	originBytes, headerCount, headerBytes := 32*1024, 100, 32*1024
	if policy != nil {
		originBytes, headerCount, headerBytes = policy.originBytes, policy.headerCount, policy.headerBytes
	}
	origin, status := nagiCORSRequestOrigin(request.Header, originBytes)
	if status != 0 {
		return status
	}
	names, ok := nagiCORSRequestedHeaders(request.Header, headerCount, headerBytes)
	if !ok {
		return http.StatusBadRequest
	}
	if policy == nil || !slices.Contains(policy.origins, origin) {
		return http.StatusForbidden
	}
	for _, name := range names {
		if !slices.Contains(policy.headers, name) {
			return http.StatusForbidden
		}
	}
	headers.Set("Cache-Control", "no-store")
	headers.Set("Access-Control-Allow-Methods", method)
	headers.Set("Access-Control-Max-Age", policy.maxAge)
	if len(names) > 0 {
		sort.Strings(names)
		headers.Set("Access-Control-Allow-Headers", strings.Join(names, ", "))
	}
	return 0
}

func nagiHTTPExtraHeaders(headers http.Header) (int64, int64) {
	var bytes, count int64
	for name, values := range headers {
		for _, value := range values {
			bytes += int64(len(name) + len(value) + len(": \r\n"))
			count++
		}
	}
	return bytes, count
}
