# Ownership

[Contents](README.md) · [Guide](language-guide.md#4-borrow-with-view-when-you-only-need-to-read) · [Types](types.md)

To give a function a string it may keep, pass an owned value. In Python, assigning a list to another name makes both names refer to the same list. Nagi distinguishes giving a value away, lending it for reading, and sharing ownership.

```nagi
def use_name(name: str):
    print(name)

def main():
    name = "alice"
    use_name(name)
    # print(name) would use a moved value
```

This complete program prints `alice`. After the call, the original `name` cannot use that string. Giving the value away is called a **move**; its new owner also takes responsibility for cleanup. Moving is not itself closing or destroying the value.

Uncommenting `print(name)` makes `check` reject use after move. If the function only reads, accept `view[str]` and pass `view(name)`. If it needs an independent string, pass `copy(view(name))`. See the [runnable borrowing example](language-guide.md#4-borrow-with-view-when-you-only-need-to-read).

## Assignment today and the planned change

The current `a = b` copies values classified as Copy, such as numbers and bools. For a non-Copy owned value, such as a string or list, it moves the value. This does not behave like Python's reference assignment.

```nagi
def main():
    count = 2
    same_count = count
    print(count + same_count)
    name = "Nagi"
    destination = name
    print(destination)
    name = "new"
    print(name)
```

Output: `4`, `Nagi`, `new`. After `destination = name`, the old string is available through `destination`; the original `name` can be used again after receiving a new value. Reading `name` before that reassignment would fail `check`. To retain both strings, use `destination = copy(view(name))` instead.

Current Copy rules also cover views, function values (including supported local async function aliases), UUIDs, timestamps, and classes/enums/nullable/owned values whose contents meet the Copy rules. Result and shared remain non-Copy even when their payloads are Copy. Copying a function value does not allow storing its Future, nor does the rule cover every enum or small class. See [types](types.md) and [async](async.md).

The adopted direction for a future migration is to require an explicit operation when assigning an existing non-Copy owned value with `a = b`: move to give it away, view to read it, copy to create an independent value, or shared ownership to retain the same value in several places. **The rules are specified and are being implemented and validated on a separate branch; current main still accepts implicit moves.** The adopted `std.ownership.move` covers ordinary assignment of an owned non-Copy local itself. Copy policy, fresh construction, and existing argument, return, and field/index consumption remain unchanged. Main integration and release availability are separate steps. See [design decisions](../../DESIGN.en.md).

## Retain the same value in several places

To retain a shared value instead of making independent copies, use `share` and `clone_shared`. Python's two-name reference assignment does not require these explicit operations. This complete Nagi example uses a class with a string field:

```nagi
class Label:
    text: str

def main():
    label = share(Label(text="Nagi"))
    another = clone_shared(label)
    duplicate = copy(view(another.text))
    print(label.text)
    print(duplicate)
```

Output: `Nagi`, `Nagi`. `share(value)` takes ownership and returns `shared[T]`. `clone_shared(label)` creates another handle to the same value without copying the payload. `copy(view(another.text))` instead creates an independent owned string.

Trying `text = another.text` would move a non-Copy field out of shared data, so `check` rejects it. Read the field, or copy it as above. Ordinary shared access does not permit arbitrary writes; resources with internal state need APIs that define their synchronization and operation rules. Sharing does not make every T thread-safe, and some resources cannot be shared or independently copied. See [built-ins](builtins.md#sharing-and-type-sizes) and [concurrency](concurrency.md).

The last shared handle releasing its value is distinct from an external service completing shutdown. Reference cycles can also retain shared values; there is no promise that arbitrary sharing graphs are automatically collected. Use the resource's explicit shutdown/close API when completion matters.

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

At the top level of a block that ends in `return`, a view can temporarily borrow local data, then be restored to an input view and returned. Generated Rust gives each assignment its own inferred borrow lifetime. Updates used after a branch or by later loop iterations remain mutations; this adds no owned-value copies. The [execution tests](../../compiler/tests/view_branch_rebinding.rs) cover High, saved Low, and handwritten Low. See the [Changelog](../../CHANGELOG.md) for release availability.

Lists, Results, and Options containing views can temporarily hold local views, restore input views, and be returned. This covers moves through aliases, nested values, `if`/`match` branches, `for`/`while`, and async functions. Generation gives successive values separate storage, evaluates the RHS before releasing the old value, and adds no owned-value copies to extend lifetimes. The [generation/execution tests](../../compiler/tests/view_flow_completion.rs) and [allocation/cleanup tests](../../compiler/tests/view_container_drop.rs) cover High, saved Low, and handwritten Low. See the [Changelog](../../CHANGELOG.md) for release availability.

The [real-runtime scope tests](../../compiler/tests/scope_runtime_contract.rs) also cover this pattern inside scopes: normal completion, error propagation, parent cancellation, and body panic. Normal error exits await child cancellation. Dropping the parent Future or unwinding requests child termination, but synchronous destruction alone cannot await completion.

Restoring a view does not make every program valid. The checker still rejects escaping local borrows. For example, returning a local view-containing binding requires a tracked origin even when its current value is empty. User-defined classes/enums containing views and arbitrary async function values remain unsupported.

Nagi's checker provides move and borrow diagnostics at Nagi source locations. It does not replace the checker for generated Rust. Nagi can also conservatively reject code that Rust would accept.

Matching Rust adapter signatures, the `Clone` required by `copy`, `Send` for async work, and `Sync` for shared state are also ultimately checked by `build`. Wrapping T in `shared[T]` does not itself make T suitable for concurrent use. Both Nagi and Rust checks must pass to produce an executable.

To keep an independent owned copy of the original data, write `copy(view(data))`.
