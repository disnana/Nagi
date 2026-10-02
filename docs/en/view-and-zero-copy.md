# Read without copying with view

A `view` lets you borrow a string or list for reading. It does not copy the data or take ownership. It is usable only while the original data remains available.

```nagi
def main() -> Result[unit, Error]:
    text = "Nagi language"
    borrowed = view(text)
    first = try slice(borrowed, 0, 4)
    print(first)
    saved = copy(first)
    print(saved)
    return ok(print(text))
```

Save this as `view.nagi` and run `nagic run view.nagi`. It prints `Nagi`, `Nagi`, and `Nagi language`. `first` borrows part of the original string. `saved` is a separate owned string created with `copy`.

## Borrowing a range

`slice(view(data), start, end)` borrows from `start` up to, but not including, `end`. String positions are UTF-8 byte offsets. An out-of-range position or an offset inside a character returns a Result error.

To store a view, first put the original value in a variable, as above. Moving or changing the original value is restricted while it is borrowed. See [Ownership](ownership.md) for examples.

`saved = view("Nagi")` fails because it stores a borrow of a temporary string. An immediate call such as `print(view("Nagi"))` is valid because it finishes using the borrow within that call.

Copying a list requires its elements to implement Rust's `Clone`. Generated classes receive Clone only when they are Copy classes. To copy a list of classes that own strings, for example, provide a Clone implementation in a Rust adapter. This implementation is verified by `build`.

## Returning a view

A function can return its input view or part of it.

```nagi
def identity(data: view[str]) -> view[str]:
    return data
```

It cannot return a view into a string created inside that function: the original data disappears when the function ends. Return an owned value when the caller needs to retain the data. Storing views in class fields or passing them to another task is not supported.

An HTTP `body: view[bytes]` also borrows received data. Creating that view adds no copy, but receiving data and decoding JSON still involve other work. See [Reading benchmarks](performance.md) for measurement details.
