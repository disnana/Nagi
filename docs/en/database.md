# SQLite

[Contents](README.md) · Previous: [HTTP and HTML](http.md) · Arguments: [Built-in functions](builtins.md)

## First reads and writes

Define the stored type with a class, open a database with `db_open`, and create a table with `db_exec`. Database operations are async and can fail, so use `try await`.

Save the following code as `database.nagi` and run `nagic run database.nagi`. It inserts one row in memory and prints `Nagi`, then `Saved`.

```nagi
class User:
    id: i64
    name: str
    age: i32

async def main() -> Result[unit, Error]:
    db = try await db_open(":memory:")
    try await db_exec(db, "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER NOT NULL)")
    user = try await db_insert[User](db, "INSERT INTO users(name, age) VALUES (?1, ?2) RETURNING id, name, age", "Nagi", 18)
    print(user.name)
    return ok(print("Saved"))
```

Use `db_open("app.sqlite")` to persist data. Relative database paths use the program's working directory, unlike imports and include_text, which use the source file's location.

`[User]` specifies the return type. Match `RETURNING` column names and types to the class. Bind values to placeholders such as `?1`; do not concatenate them into SQL.

| Operation | Form | Return type after await |
|---|---|---|
| Create a table, etc. | `db_exec(db, sql)` | `Result[i64, Error]` |
| Read all rows | `db_all[User](db, sql)` | `Result[List[User], Error]` |
| Read one row | `db_query[User](db, sql, id)` | `Result[User?, Error]` |
| Insert | `db_insert[User](db, sql, name, age)` | `Result[User, Error]` |
| Update | `db_update[User](db, sql, id, name, age)` | `Result[User, Error]` |
| Delete, etc. | `db_write(db, sql, id)` | `Result[i64, Error]` |

Bind argument shapes are currently fixed: query/write take one i64, all takes none, insert takes str/i32, and update takes i64/str/i32. Columns do not have to be named name/age; tables with matching argument types also work.

## Read from an API

Pass SQL values as arguments rather than concatenating them into the SQL string. Each returned row fills the fields of the specified class.

This handler fragment uses the User type above:

```nagi
async def get_user(db: Db, id: i64) -> Result[User?, Error]:
    return await db_query[User](db, "SELECT id, name, age FROM users WHERE id = ?1", id)
```

For iteration over class lists, only Copy classes are currently supported. User owns a str, so `for user in users` is not supported. An HTTP handler can still return `await db_all[User](...)` as a JSON array. See the [CRUD API](../../examples/crud.nagi) and [task management source](../../test-nagi-code/web-demo/tasks.nagi).

## Implementation and limits

A dedicated thread runs SQLite operations in order. Its queue holds up to 64 jobs. Nagi waits for the result asynchronously, while SQLite reads and writes on that thread.

Prepared statements and resolved column names are reused. Returned strings and byte sequences are owned so they remain valid after processing the SQLite row.

General variable-length typed parameters, transaction APIs, database pools, and compile-time schema checking are not implemented. Column names, SQL, and column type mismatches produce runtime Result errors.

A database job already accepted may complete and commit even after its HTTP caller times out. Cancelling the caller does not guarantee that a write is rolled back.
