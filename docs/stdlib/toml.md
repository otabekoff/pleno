# std:toml

> **Stability:** stable in 1.0.

TOML parsing and serialization.

## Import

```pleno
import { TOML } from "std:toml";
```

`parse` and `stringify` can also be imported directly.

## Functions

### `parse(toml: String): Map`

Parses a TOML string into a Map. TOML dates and times become strings. Invalid TOML throws an error.

```pleno
import { TOML } from "std:toml";

let config = TOML.parse(`
[database]
host = "localhost"
port = 5432

[server]
workers = 4
`);

print(config.database.host);  // localhost
print(config.server.workers); // 4
```

### `stringify(value: Any): String`

Converts a Map to a TOML string. Nested maps become tables. TOML has no null, so `null` values and functions throw an error.

```pleno
import { TOML } from "std:toml";

let config = {
    "title": "app",
    "database": {
        "host": "localhost",
        "port": 5432
    }
};

print(TOML.stringify(config));
// title = "app"
//
// [database]
// host = "localhost"
// port = 5432
```

---
