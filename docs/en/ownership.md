# Ownership

Passing str, bytes, List, or a class with owned fields to a function moves it. The checker rejects use of its name after the move. Copy values can be used repeatedly.

```nagi
def use_name(name: str):
    print(name)

def main():
    name = "alice"
    use_name(name)
    # print(name) would use a moved value
```

## Taking a class field

Assigning a string field to another variable moves ownership of that string. You cannot reuse the moved field, but the other fields remain available.

```nagi
class Person:
    name: str
    age: i64

def main():
    person = Person(name="Nagi", age=1)
    name = person.name
    print(name)
    print(person.age)
    # print(person.name) would use a moved value
```

To keep the original field too, change the assignment above to `name = copy(view(person.name))`. Make the copy before moving the value. Reading a string with `print` or `len` does not move it.

For a function that only needs to read, see [Borrow with view](language-guide.md#4-borrow-with-view-when-you-only-need-to-read).

## Using a value in a loop

Passing a string created outside a loop as an owned argument moves it on the first iteration. It is then unavailable on the next iteration, so `check` rejects this reuse. The same rule applies to taking fields and passing owned values in a `while` condition.

To keep the original value, make a copy on each iteration. For a function that only reads, declare its argument as `view[str]` and pass `view(name)`.

```nagi
def use_name(name: str):
    print(name)

def main():
    name = "Nagi"
    for number in range(2):
        use_name(copy(view(name)))
    print(name)
```

You can also assign a new value before the next iteration.

```nagi
def use_name(name: str):
    print(name)

def main():
    name = "first"
    for number in range(2):
        use_name(name)
        name = "next"
    print(name)
```

If reassignment depends on a branch, every path that continues to the next iteration must provide a value. A path that ends the function with `return` does not affect the next iteration or subsequent statements. Values created inside the loop are available anew on each iteration.

The checker accounts for repetition and zero iterations without calculating the iteration count. An assignment inside a loop therefore does not always make a value available after the loop. A `while` condition is also evaluated when the loop exits.

## Borrowing and the limits of checking

`for value in values` borrows the array being iterated. On paths that continue to the next iteration, `check` rejects appending to, reassigning, or moving that array. This also applies when iterating through `view(values)` or a variable holding that view. Other arrays can be changed, and the iterator's borrow ends when the loop finishes.

```nagi
def main():
    values = [1, 2]
    output: List[i64] = []
    for value in values:
        append(output, value * 2)
    append(values, 3)
```

Field borrows are tracked by place. While `view(data.values)` is live, you can still move a separate field such as `data.name`. Moving all of `data` or its `values` field is rejected.

Moving, reassigning, or appending to an owned value is also restricted while a view borrows it. Borrows are tracked conservatively by lexical scope. This does not yet match Rust's non-lexical lifetime analysis.

The Nagi checker alone does not establish soundness for partial field moves, complex branches and loops, or generic borrows. Code generation emits safe Rust; only programs accepted by the backend's borrow checker become executables. A successful `check` and a successful `build` are different guarantees.

Ownership inference aims to show where copies are needed without adding lifetime notation to application code. To retain another owned copy, write `copy(view(data))`.
