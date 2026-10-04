# Stock report CLI

Read one JSON line and report available and reserved inventory. `inventory.nagi` contains the validation rules and error type. The project uses no Rust adapter.

From the repository root, start the application and enter the JSON below.

```sh
nagic run --project test-nagi-code/application-examples/stock-report/nagi.toml
```

```json
{"warehouse":"東京倉庫","items":[{"product_id":1,"on_hand":12,"reserved":3},{"product_id":2,"on_hand":5,"reserved":5}]}
```

Application output:

```json
{"warehouse":"東京倉庫","products":2,"available":9,"reserved":8}
```

Product IDs must be positive integers. Each record supports 0–1,000,000 units, with reservations no greater than stock; a batch supports at most 10,000 records. `products` counts input records, including repeated product IDs. Warehouse names must contain 1–80 UTF-8 bytes.

Invalid input writes a reason to stderr and exits with code 1. `InventoryError` distinguishes JSON decoding failures from validation failures; the CLI entry point converts these into readable messages.

`std.result` converts the Error from JSON decoding. A successful `Batch` passes through; only a failure calls `invalid_json`.

```nagi
import std.result as result

def invalid_json(cause: Error) -> InventoryError:
    return InventoryError.InvalidJson(cause)

def parse(text: view[str]) -> Result[Batch, InventoryError]:
    return result.map_error(json_decode[Batch](text), invalid_json)
```

The report calculation still uses `batch = try parse(text)`. See the [error-handling reference](../../../docs/en/error-handling.md) for the helper's contract.

`smoke.py` sends valid data, boundary values, empty inventories, Japanese text, malformed JSON, and incorrect field types to the executable. It checks the JSON values and exit codes. See the [parent README](../README.en.md) for the shared verification command.
