# Group data with classes

Use a `class` to group related values, such as coordinates or a user's name and age. Declare each field's name and type, then provide every field when constructing a value.

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

Classes compile to Rust structs. A list of numeric classes does not need a separate allocation for every element. String and list fields still need storage for their data. Nagi does not define a fixed memory layout for a C interface.
