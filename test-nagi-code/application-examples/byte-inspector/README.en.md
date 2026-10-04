# HTTP byte inspector

[日本語](README.md)

POST a body to receive its byte count, the sum of its bytes, and its first byte as JSON. An empty body returns `null` for `first`.

Nagi reads `view[bytes]` without copying the input and processes the `u8` values obtained through iteration and indexing. The standard library handles HTTP transport. The application needs no handwritten Rust or database.

This example requires Nagi 0.1.10 or later for the byte-view iteration and indexing fixes. The following command builds the compiler from repository source and runs the example:

```sh
cargo run -p nagic -- run --project test-nagi-code/application-examples/byte-inspector
```

In another terminal:

```sh
curl --data-binary 'ABC' http://127.0.0.1:8096/bytes
```

```json
{"length":3,"total":198,"first":65}
```

Use `curl.exe` on Windows. The default port is 8096; set `NAGI_SAMPLE_PORT` to change it. The body limit is 4096 bytes. The sum is not suitable for authentication or tamper detection.

To verify both High and independently loaded saved Low over HTTP:

```sh
cargo build -p nagic
python scripts/verify_application_examples.py --compiler target/debug/nagic --only byte-inspector
```

The checks cover empty bodies, all byte values, UTF-8, invalid UTF-8, the maximum accepted size, HEAD, unsupported methods, and missing routes.
