# std:toml

TOML parsing and serialization.

## Import

```pleno
import { TOML } from "std:toml";
```

## Functions

### `parse(toml: String): Map`

Parses a TOML string into a Pleno value.

```pleno
let config = TOML.parse(`
[database]
host = "localhost"
port = 5432

[server]
workers = 4
`);

print(config.database.host);  // "localhost"
print(config.server.workers); // 4
```

### `stringify(value: Any): String`

Converts a Pleno value to a TOML string.

```pleno
let config = {
    "database": {
        "host": "localhost",
        "port": 5432
    }
};

let toml = TOML.stringify(config);
```

---
