# Read and write JSON

Nagi checks the mapping between JSON and Nagi types; Serde/serde_json perform the conversion. Define a class for the data you want to read. `json_decode[User](text)` turns JSON into a `User`; `json_encode(user)` turns it back into a JSON string.

```nagi
class User:
    name: str
    age: i32

def main() -> Result[unit, Error]:
    user = try json_decode[User](
        "{\"name\":\"Nagi\",\"age\":20}"
    )
    print(user.name)
    encoded = try json_encode(user)
    return ok(print(encoded))
```

Save this as `json.nagi` and run `nagic run json.nagi`. It prints `Nagi` and JSON containing the name and age. Both operations can fail, so `try` returns errors to the caller. See [Error handling](error-handling.md).

## Types and borrowing

Besides classes, you can use JSON-compatible numbers, bool, str, List, and other data types. Functions, Error, enums, sqlite.Pool, and Html cannot be encoded or decoded. `check` also rejects them inside classes or containers such as List and Result.

`json_decode[view[str]](text)` borrows a JSON string from the input. If you store the result, keep the input in a variable and do not move or replace it while borrowed. Use `json_decode[str](text)` when you need an independent owned string.

Borrowed decoding requires a string that can refer directly to the input. JSON escapes such as `\n` or `\uXXXX` require expansion, so borrowed decoding returns an error for them. Decode these strings as `str` instead.

`bytes` reads and writes an integer array such as `[97,98,99]`. In contrast, `json_decode[view[bytes]]` borrows UTF-8 bytes from an unescaped JSON string such as `"abc"` and cannot read an integer array. `json_encode` also writes borrowed bytes as an integer array; use `bytes` to read and write the same representation.

## Input validation

Missing required fields, extra fields, incorrect types, out-of-range numbers, and invalid UTF-8 return errors. For example, an `age: i32` field cannot accept a string or an integer outside its range.

JSON is read directly into the requested type. String fields own their data and allocate storage during decoding. Borrowed class fields and settings for default values are not supported.

Currently, `json_encode` writes NaN and positive or negative infinity as `null` rather than returning an error, including values inside Lists and classes. That `null` cannot be decoded as `f64`; an encoded non-finite `f64?` value decodes as `None`.

See [HTTP and HTML](http.md) for JSON requests and responses.

The implementation is in the [JSON runtime](../../runtime/src/lib.rs) and [generated class conversions](../../compiler/src/emit.rs). [Type contract tests](../../compiler/tests/builtin_type_contracts.rs) check accepted types and borrowing restrictions; the [runtime tests](../../runtime/src/lib.rs) check invalid input and field validation.
