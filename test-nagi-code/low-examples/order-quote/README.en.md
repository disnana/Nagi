# Order quote CLI in handwritten Low

Read one JSON line, validate quantities and integer prices, and return the subtotal, discount, and total as JSON. The entry point, `main.low`, imports `quote.low`. Both files are handwritten Low; the project uses no Rust adapter.

Check and start the application from the repository root. `run` requires Rust/Cargo.

```sh
nagic check --project test-nagi-code/low-examples/order-quote
nagic run --project test-nagi-code/low-examples/order-quote
```

After startup, enter this JSON on one line.

```json
{"customer":"東京","items":[{"product_id":1,"quantity":3,"unit_price_cents":2500},{"product_id":2,"quantity":5,"unit_price_cents":500}]}
```

Output:

```json
{"customer":"東京","lines":2,"units":8,"subtotal_cents":10000,"discount_cents":500,"total_cents":9500}
```

Prices use integer minor currency units. A subtotal of at least 10,000 receives a 5% discount, rounded down. Input supports 1–1,000 lines, quantities of 1–1,000, and unit prices of 0–1,000,000. Product IDs must be positive integers; customer names must contain 1–80 UTF-8 bytes. `lines` counts input lines, including repeated product IDs. These limits keep the maximum subtotal at 1,000,000,000,000, within `i64`.

The records in `quote.low` define the JSON structure. `QuoteError` distinguishes JSON decoding failures from each validation failure. `main.low` matches the Result, writes a reason to stderr for invalid input, and exits with code 1. Success writes just one JSON line to stdout.

Local variables omit type annotations, as in `let subtotal = 0;`. Record fields, function parameters, and return values declare their types. `view(text)` and `view(order.customer)` borrow strings for reading. The `Order` moves into `calculate`, which later moves its `customer` field into the output `Quote`. Low uses the same type and ownership checks as High.

The line record contains only integer fields, so `for line in order.items` copies each value. Records with non-Copy fields such as strings can also be iterated through read-only borrows. Indexing non-Copy elements and views of whole records are not yet supported.

`smoke.py` sends 23 cases to the executable and checks JSON values, integer types, and exit codes. It covers valid data, Japanese text, discount boundaries and rounding, maximum values, empty arrays, malformed JSON, and incorrect types. Run it from this directory.

```sh
nagic build
python3 smoke.py --executable "/path/to/executable"
```

Replace `/path/to/executable` with the path printed to standard error in the build's `native:` line. Successful executables are stored by build generation under the generated directory's `.nagi/`. This layout targets Nagi 0.1.11; check the official release record to confirm published availability. `NAGI_NATIVE_TARGET_DIR` selects a shared dependency cache; it does not change the executable location. Do not construct the filename or generation path. On Windows, use the `.exe` path printed in `native:`.

[日本語](README.md) · [High and Low](../../../docs/en/low-language.md) · [JSON](../../../docs/en/json.md)
