# JSON and typed classes

`json_decode[User](input)` uses Serde's typed deserialization. It does not first read everything into a generic Value, convert that to a dictionary, and rebuild a model. Missing fields, type mismatches, out-of-range i32 values, unknown fields, and invalid UTF-8 are rejected.

```nagi
def read_user(text: str) -> Result[User, Error]:
    return json_decode[User](text)
```

Owned String fields require allocations and copies. The class itself and primitive fields are native values. Experiments with `&str` borrowed from input have confirmed zero allocations, but borrowed class fields are not available in High yet.

`json_encode` encodes a class into an owned String. HTTP responses use the same typed serialization into `Vec<u8>`. Runtime comparisons also measure encoding with `to_writer` into a warm buffer. This buffer reuse is not automatically applied to HTTP responses.

Rules for NaN/Infinity, an option to allow unknown fields, nullable and default values, and direct encoding of response literals remain to be specified.
