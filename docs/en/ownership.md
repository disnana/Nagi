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

## Borrowing and the limits of checking

Moving, reassigning, or appending to an owned value is also restricted while a view borrows it. Borrows are tracked conservatively by lexical scope. This does not yet match Rust's non-lexical lifetime analysis.

The Nagi checker alone does not establish soundness for partial field moves, complex branches and loops, or generic borrows. Code generation emits safe Rust; only programs accepted by the backend's borrow checker become executables. A successful `check` and a successful `build` are different guarantees.

Ownership inference aims to show where copies are needed without adding lifetime notation to application code. To retain another owned copy, write `copy(view(data))`.
