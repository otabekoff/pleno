# std:db

> **Stability:** experimental. Not covered by the [1.x compatibility promise](../specs/compatibility.md).

SQLite databases, through `Database.connect`. SQLite is built into `pleno`; there is nothing else to install.

::: warning Experimental
Only SQLite is supported, results are plain maps, and there are no helpers for transactions and no prepared statement objects yet. The API may change in any 1.x release.
:::

## Import

```pleno
import { Database } from "std:db";
```

## `Database.connect(connection: String): Database`

Opens a database. The connection string is `sqlite:` followed by a file path, relative to the working directory; the file is created if it does not exist. `sqlite::memory:` opens a new database that lives in memory and is gone when it is closed or the program ends.

Any other connection string throws `Unsupported database driver. Use 'sqlite:path'`. A file the sandbox does not allow throws a `SecurityError`: the database file must be readable under `--allow-read` and writable under `--allow-write`, and `--allow-io false` denies every database (see [Security](../specs/security-features.md)).

```pleno norun
import { Database } from "std:db";

let db = Database.connect("sqlite:app.db");   // app.db in the working directory
```

The returned value has three methods.

### `execute(sql: String, params?: Array): Int`

Runs a statement that returns no rows (`CREATE`, `INSERT`, `UPDATE`, `DELETE`, ...) and returns the number of rows it changed.

### `query(sql: String, params?: Array): Map[]`

Runs a `SELECT` and returns its rows, each a map from column name to value, in the order the query gives them.

### `close(): Null`

Closes the database. Calling `execute` or `query` afterwards throws `Database connection is closed`. Closing twice is allowed.

```pleno
import { Database } from "std:db";

let db = Database.connect("sqlite::memory:");
db.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, score REAL)");
print(db.execute("INSERT INTO users (name, score) VALUES (?, ?)", ["Ada", 9.5]));   // 1
db.execute("INSERT INTO users (name, score) VALUES (?, ?)", ["Alan", null]);

print(db.query("SELECT id, name, score FROM users ORDER BY id"));
// [{"id": 1, "name": "Ada", "score": 9.5}, {"id": 2, "name": "Alan", "score": null}]

print(db.query("SELECT name FROM users WHERE score > ?", [5]));   // [{"name": "Ada"}]
db.close();
```

## Parameters

Write `?` in the SQL for each value and pass the values, in order, as an array. Values are passed to SQLite separately from the SQL text, so they cannot change the statement: always use parameters for values that come from outside the program, never string concatenation.

| Pleno value | SQLite value |
| --- | --- |
| `null` | `NULL` |
| `Int` | `INTEGER` |
| `Float` | `REAL` |
| `String` | `TEXT` |
| `Bool` | `INTEGER` `1` or `0` |
| array of Ints from 0 to 255 | `BLOB` |

Any other value throws `Unsupported parameter type in SQL`.

## Results

| SQLite value | Pleno value |
| --- | --- |
| `NULL` | `null` |
| `INTEGER` | `Int` (a Bool column comes back as `1` or `0`) |
| `REAL` | `Float` |
| `TEXT` | `String` |
| `BLOB` | an array of Ints, one per byte |

## Errors

SQL that SQLite rejects, such as a syntax error or an unknown column, throws an `Error` whose message starts with what failed (`Prepare failed: ...`, `Execute failed: ...`) followed by SQLite's own description.

```pleno
import { Database } from "std:db";

let db = Database.connect("sqlite::memory:");
try {
    db.query("SELECT missing FROM nowhere");
} catch (e) {
    print(e.message.startsWith("Prepare failed"));   // true
}
```
