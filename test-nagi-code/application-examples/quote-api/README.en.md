# Typed JSON quote API

[日本語](README.md)

This quote API uses `std.http.server` without a database. It decodes request JSON into `QuoteInput`, validates the quantity and product, and returns a `Quote`. The App owns a typed `Config`; each handler reads it through `shared[Config]`.

Start it from the repository root. You need Nagi 0.1.9 and a working Rust/Cargo build environment.

```sh
nagic run --project test-nagi-code/application-examples/quote-api
```

The default address is `http://127.0.0.1:8092`. Set `NAGI_SAMPLE_PORT` to change the port and `NAGI_SAMPLE_MAX_QUANTITY` to change the maximum quantity. The maximum must be 1–1000 and defaults to 1000. Invalid configuration fails before the server starts. Press Ctrl+C to stop it.

```sh
curl http://127.0.0.1:8092/health
curl -H 'Content-Type: application/json' \
  -H 'X-Request-ID: 123e4567-e89b-12d3-a456-426614174000' \
  -d '{"sku":"NOTEBOOK","quantity":2}' \
  http://127.0.0.1:8092/quotes
```

A successful quote returns 200 with `application/json`.

```json
{"sku":"NOTEBOOK","quantity":2,"currency":"USD","unit_price_minor":1250,"subtotal_minor":2500,"shipping_minor":500,"total_minor":3000}
```

Amounts are integers in USD cents. A NOTEBOOK costs 1250, shipping costs 500, and quantities of five or more receive free shipping. The quantity is bounded before calculation, so these configured amounts fit in `i64`.

| Request | Response |
| --- | --- |
| `GET /health` | 200; JSON containing `status`, `currency` and `maximum_quantity` |
| `HEAD /health` | The GET status and headers, without a body |
| `POST /quotes` | 200; quote JSON |
| Invalid JSON, missing fields, extra fields or incorrect types | 400, `invalid_json` |
| Invalid, duplicate or non-UTF-8 `X-Request-ID` | 400, `invalid_request_id` |
| Duplicate or malformed `Content-Type` | 400, `invalid_header` |
| Missing `Content-Type` or a value other than `application/json` | 415, `unsupported_media_type` |
| Quantity outside the configured range | 422, `invalid_quantity` |
| SKU other than `NOTEBOOK` | 422, `unknown_sku` |

The sample uses `http.is_json_content_type`. It accepts case variations, surrounding spaces or tabs, and parameters such as `application/json; charset=utf-8`. Parameters do not select a decoder; the body must be UTF-8. JSON must contain exactly `sku: str` and `quantity: i64`; numeric strings and fractional quantities are rejected. If both business inputs are invalid, quantity validation runs first.

Application input and business errors use this body shape. JSON decoding details are not exposed in responses.

```json
{"code":"invalid_quantity","message":"Quantity is outside the configured range"}
```

`X-Request-ID` is optional. When supplied, it must be one UUID. The API returns its canonical value with successful quotes and application errors. It does not generate IDs. Invalid IDs are not echoed. `/health` does not process this header.

Unknown routes return 404. Unregistered methods such as `GET /quotes` return 405 with `Allow: POST`.

The HTTP receiver limits bodies to 4096 bytes. An oversized body is rejected with 413 and `Connection: close`, without calling the handler. The server closes without draining the remaining body, which can cause a TCP reset. Receipt of the 413 is not guaranteed for every OS, client, or upload pattern.

These server responses bypass the application mapper, so they do not guarantee the JSON error shape or request ID echo described above. Response construction failures return 500; the final fallback has no body.

`calculate` returns a custom `Result[Quote, QuoteError]`. `std.result.map_error` converts JSON and HTTP errors into `QuoteError`. The handler retains the request ID while `quote_response` runs. On failure it moves the ID into `ApiFailure`, which the App mapper converts into a response; each processing stage does not need its own ID copy. `/health` uses the built-in `Error` and a separate mapper registered with `route_mapped`.

`smoke.py` exposes the same verification function for High and saved Low. It checks quotes, the free shipping boundary, typed JSON rejection, headers and UUIDs, shared configuration, server limits and listener release on localhost. See the [parent README](../README.en.md) for the shared verification command.

The sample only calculates quotes. It does not store quotes, reserve stock, calculate tax, process payments or authenticate users. The standard server supports loopback HTTP/1.1.
