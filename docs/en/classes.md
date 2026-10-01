# Classes and value layout

A class defines a type. Using the word `class` does not itself require heap allocation.

```nagi
class Point:
    x: f64
    y: f64
```

A class containing only primitives generates a Rust Copy value type. `List[Point]` is a `Vec<Point>`; each Point is not a separate heap object. `examples/values.nagi` iterates over two Points and prints their sum, 10, and the type size.

Classes containing String or List are also values, but their fields own data that requires allocation. This distinction makes layout and copying costs easier to assess.

Version 0.1 supports fields and construction with named arguments. Methods, interfaces, traits, and inheritance are not implemented. Use composition and free functions. Borrowed fields are rejected while lifetime parameter rules remain undecided.

High and Low share the generated layout within the same Rust build. It uses ordinary Rust struct layout and does not declare a stable C ABI layout.
