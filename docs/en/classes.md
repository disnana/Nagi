# Group data with classes

A `class` is a value type with named fields. Use it to group related values, such as coordinates or a user's name and age. Declare each field's name and type, then provide every field when constructing a value.

```nagi
class Point:
    x: f64
    y: f64

def distance_squared(point: Point) -> f64:
    return point.x * point.x + point.y * point.y

def main():
    point = Point(x=3.0, y=4.0)
    print(point.x)
    print(distance_squared(point))
```

Save this as `point.nagi` and run `nagic run point.nagi`. It prints `3` and `25`. Read a field with `point.x`. Define functions that process the value outside the class.

## Passing values

Classes containing only copyable values, such as numbers, can be used repeatedly. `Point` above is one example. You can also read elements of `List[Point]` in a `for` loop.

Passing a class containing strings or lists to a function moves its ownership. See [Ownership](ownership.md) if you need to use it afterward.

## Supported operations

You can define fields, construct values with named arguments, and read fields. Methods, inheritance, field assignment, and storing a `view` in a field are not supported.

Fields can contain Error or enums, including an Error cause in a custom error class. Db and storable standard resources can also be state fields. Classes containing Error, enums, or resources cannot be converted to JSON. Function types and Html are unsupported in fields, including inside lists or nullable types. Map fields cannot use keys such as f32, f64, UUID, or timestamp that lack hashing support.

A `shared[T]` field can be converted to JSON when T supports JSON. JSON contains the value of T; decoding creates a new shared value. It does not preserve which values originally shared the same reference.

Classes compile to Rust structs. A list of numeric classes does not need a separate allocation for every element. String and list fields still need storage for their data. Nagi does not define a fixed memory layout for a C interface.

A class cannot contain itself directly as a field. Wrapping it in `Node?` or `Result[i64, Node]` still produces an undefined value size and is an error. Use indirect storage for child nodes, such as `children: List[Node]`.
