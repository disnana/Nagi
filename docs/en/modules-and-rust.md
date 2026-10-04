# File imports and Rust integration

Load another Nagi file with a quoted relative path. Use `import "filename" as name` to give it a module name, or `from "filename" import definition` to select a function, class, or enum. To call Rust, declare the function's parameter and return types in Nagi and supply a Rust file when building.

The module and from-alias features below are available from Nagi 0.1.8.

## Split Nagi code into files

Create two files in the same directory. Put this class in `models.nagi`:

```nagi
class Item:
    name: str
    count: i64
```

Import it from `app.nagi`:

```nagi
import "models.nagi"

def main():
    item = Item(name="Nagi", count=42)
    print(item.name)
    print(item.count)
```

```sh
nagic run app.nagi
```

The program prints `Nagi` and `42`. Paths are relative to the file containing the import. High imports `.nagi`; Low imports `.low`. Low allows a semicolon after an import.

This traditional import loads definitions from the file and its dependencies into the same namespace. Built-in functions remain available as before.

### Module names and definition aliases

Put these definitions in `orders.nagi`:

```nagi
class Order:
    amount: i64

def score(order: Order) -> i64:
    return order.amount
```

Use a module name and a class alias in `app.nagi`:

```nagi
import "orders.nagi" as orders
from "orders.nagi" import Order as SavedOrder

def main():
    order: SavedOrder = orders.Order(amount=42)
    score = orders.score
    print(score(order))
```

`orders.Order` and `SavedOrder` are the same type. You can also call `orders.score(order)` directly. Qualified names work in type arguments, field types, and nullable types, such as `List[orders.Order]` and `orders.Order?`. An `Order` defined in another file is a different type; passing one where the other is required is a type error.

A module name exposes functions, classes, and enums defined in that file. Imported names are not automatically re-exported. The alias is optional in `from "orders.nagi" import Order`. Select several definitions with commas, for example `from "orders.nagi" import Order as SavedOrder, score`. Do not add a trailing comma. `from` and `as` are contextual import keywords and can still be function or variable names. A local with the same name as a module follows the existing local-variable rules. Class method calls remain unsupported.

Each real file is loaded once, even through several module names, from aliases, or traditional imports. Import cycles, missing files, and mixed High/Low files are errors. A from import of a missing definition, or an import that gives different definitions the same name in one scope, reports an error at that import. Unquoted imports select the registered `std.http.server`, `std.actor`, and `std.result` modules. General package discovery and visibility declarations are unsupported.

### Use the same definition in Low and Rust

Low also accepts `import "orders.low" as orders;` and `from "orders.low" import Order as SavedOrder;`. Generated Low retains module and definition IDs, so reparsing it or integrating handwritten Low preserves type identity.

To replace a function reached through the root module name `orders`, use `@replace generated::orders::score` in handwritten Low. Traditional `@replace generated::score` remains supported. Parameter, return-type, and async requirements follow the existing [Low replacement rules](low-language.md).

Rust adapters refer to a class from a root module import as `super::orders::Order`, or through its from alias as `super::SavedOrder`. Both paths refer to the same generated type. Traditional flat imports still expose paths such as `super::Item`. Internal generated names avoid collisions while JSON field names and SQL column names stay unchanged. The [module example](../../test-nagi-code/library-examples/module-imports/README.en.md) demonstrates same-named classes and function aliases.

### Import limits and error locations

The limits are 128 files, depth 64, 8 MB total, and 2 MB per file. Type-checking and Low integration errors report original filenames and line numbers. Errors in generated Rust also show the corresponding Nagi or Low line first. If no source line can be identified, or an error is in handwritten Rust, the compiler reports the Rust location. Column mapping is not yet implemented.

### Embed a text file

`include_text("index.html")` reads a neighboring UTF-8 file at build time and embeds it as a `str`. Specify the path as a string literal. The original file is not needed when running the distributed application.

## Import the standard HTTP library

`std.http.server` and `std.actor` are standard libraries available from Nagi 0.1.8.

```nagi
import std.http.server as http
from std.http.server import Request, Response, Status as Code

def main():
    response = http.text(Code.OK, "Hello, Nagi!")
    print(response.status.value)
```

Standard modules are registered libraries provided with the compiler. A same-named file in the current directory or an editor buffer cannot replace them. A module import requires `as`; `from` selects individual definitions. Importing starts no listener or worker. Generated Low retains standard definition IDs, and aliases of a standard type refer to the same native type.

See [types](types.md#standard-http-resource-types) for resources such as Request. Each `App[State, E]` has its own state and error mapper and can run without Db. `route` accepts a named async handler or its local alias; `route_mapped` supplies a mapper for that route. General restrictions on async callback parameters and storing async functions in classes still apply. `check` validates types and ownership; the Rust build checks the handler future's `Send + 'static` and shared State's `Send + Sync`. See [HTTP](http.md) for starting a server, building responses, and working with headers.

## Import the standard actor library

Actors use the same import rules:

```nagi
import std.actor as actor
from std.actor import Actor as Worker, CallError
```

`Worker[M, R, E]` and `actor.Actor[M, R, E]` are the same native type. Register named async factories and handlers with `Supervisor[C]`, then return the next state and reply in `Turn[S, R, E]`. Messages, replies, and business errors must be owned values whose capacity can be accounted for; they cannot contain Map, views, shared graphs, or opaque resources. See [actors](actor.md), the [API reference](actor-reference.md), and the [sample](../../test-nagi-code/library-examples/supervised-service/README.en.md) for signatures and steps.

## Call Rust functions

Save this as `app.nagi`. `@rust` names the Rust function, and `extern def` declares its parameter and return types. The declaration has no body or trailing colon.

```nagi
@rust("native::text_bytes")
extern def text_bytes(text: view[str]) -> i64

def main():
    text = "Nagi"
    print(text_bytes(view(text)))
    print(text)
```

Put the Rust function in `native.rs` in the same directory:

```rust
pub fn text_bytes(text: &str) -> i64 {
    text.len() as i64
}
```

```sh
nagic run app.nagi --rust native.rs
```

The result is `4`, followed by `Nagi`. The function borrows the string through `view[str]`, so it remains available afterward. `--rust` includes the Rust file as the `native` module. Make the function `pub` and match the declared types. For example, Nagi's `str` is Rust's `String`, and `view[str]` is `&str`.

Low uses `extern fn ...;`. To call a Rust async function, use `extern async def` (`extern async fn` in Low) and await it at the call site. Rust function paths are sequences of identifiers beginning with `native::`. External functions cannot declare a view return type or carry HTTP attributes.

### Use a Rust crate

Add Cargo dependencies with `--rust-dep NAME=VERSION`. For an adapter using serde_json, pass:

```sh
nagic run app.nagi --rust native.rs --rust-dep serde_json=1.0
```

Cargo normally needs network access to download dependencies on the first build. Use a dependency table in `nagi.toml` to set a local crate `path`, select `features`, or use `package` to give a dependency a different name. The [local Rust library example](../../test-nagi-code/rust-library/README.en.md) calls an independent crate through an adapter that converts Rust structs/errors into a Nagi class/Error. See the [repository's Rust integration example](../../test-nagi-code/rust-bridge/) for a complete example using a crate. Save your entry file, Rust file, and dependencies in [nagi.toml](projects.md) to reuse them in the CLI and VS Code.

Nagi's `check` validates the declared types, calls, ownership, and borrowing. `check`, `lower`, and `symbols` do not invoke Cargo or fetch dependencies. It does not inspect Rust bodies or crate APIs. A `build` checks that the Rust implementation matches its declaration. Registered standard resources use their corresponding native types. Adapt other Rust-specific types to numbers, str, List, classes, or Result before passing them to Nagi. Refer to a generated Nagi class through the root import, using `super::TypeName` or `super::module_name::TypeName`.

To reuse the same dependency resolution, retain the generated Cargo.lock and run `cargo build --locked --manifest-path build/app/Cargo.toml`. A normal `nagic build` runs `cargo build --release` on the generated project and retains its existing lock. `nagic build --locked` is unsupported. The lock does not pin local crate source contents.

This integration calls functions within the same Rust build. A stable C ABI and runtime DLL loading are unsupported.

See [libraries and Rust assets](libraries.md) and [sample projects](library-examples.md) for shared code and callbacks passed from Nagi to Rust. The [library design](library-design.md) describes current support and proposed resource types and runtime selection.
