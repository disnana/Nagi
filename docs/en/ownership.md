# Ownership

Passing a string or list to a user-defined function transfers ownership of that data to the function. This is called a move. Reusing the original variable is an error. Copyable values, such as numbers, can be used repeatedly.

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

Copy elements, such as numbers, are copied into the loop variable. Non-Copy elements, such as classes containing strings, are borrowed for reading. Iteration does not clone their strings or records.

```nagi
class User:
    name: str
    score: i64

def main():
    users = [User(name="Nagi", score=10)]
    for user in users:
        print(user.name)
        print(user.score)
        name = copy(view(user.name))
        print(name)
    append(users, User(name="凪", score=20))
```

A borrowed element cannot be reassigned. You cannot pass or return the whole element, or a non-Copy field, as an owned value. To keep a string or array, copy the field explicitly as shown above. A field view created inside the loop cannot be stored outside it. Borrowing a non-Copy enum, nullable, or Result payload through `match` is not yet supported.

Borrowing one field leaves separate fields available. For example, while `view(data.values)` is in use, you can still take `data.name`. You cannot move all of `data` or its `values` field.

While a view borrows data, moving, reassigning, or appending to that data is restricted. The checker tracks borrows by code blocks; it does not determine the end of a borrow as precisely as Rust.

A view-containing return type can accept a directly constructed value with no borrow, such as `return None`, `return []`, or `return ok(None)`. Returning a local of a view-containing type still requires a tracked borrowing origin, even if it currently holds an empty value.

Nagi's checker cannot yet decide every case involving partial field moves, complex branches, or borrows. Generated Rust is also checked for borrowing, and a program becomes an executable only after both checks pass. A successful `check` can therefore still be followed by a failed `build`.

To keep an independent owned copy of the original data, write `copy(view(data))`.
