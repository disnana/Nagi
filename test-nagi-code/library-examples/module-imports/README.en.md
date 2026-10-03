# Module names and from aliases

`orders.nagi` and `archive.nagi` each define `Order` and `total`. The application distinguishes them through module names and also uses `SavedOrder` as an alias for `orders.Order`.

Run inside this directory. `check` and `lower` need Nagi. `run` also needs Rust/Cargo and a toolchain that can build Nagi applications.

```sh
nagic check
nagic lower
nagic run
```

The output is:

```text
42
42
7
```

`saved_total` accepts `SavedOrder`, the same type as the argument to `orders.total`. `total = orders.total` uses a module function as a value. `archive.Order` is a different type, so changing the call to `saved_total(older)` produces a type error. Matching fields do not make classes from different files interchangeable.

These classes contain only numbers, so the same value can be passed twice. Classes containing strings or lists follow the existing move rules. A module name exposes functions and classes defined in that file; imported names are not automatically re-exported.

[日本語](README.md) · [Imports and Rust integration](../../../docs/en/modules-and-rust.md)
