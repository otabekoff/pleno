# std:yaml

YAML parsing and serialization.

## Import

```pleno
import { YAML } from "std:yaml";
```

`parse` and `stringify` can also be imported directly.

## Functions

### `parse(yaml: String): Any`

Parses a YAML string into a Pleno value. Invalid YAML throws an error.

```pleno
import { YAML } from "std:yaml";

let config = YAML.parse(`
database:
  host: localhost
  port: 5432

features:
  - auth
  - logging
  - caching
`);

print(config.database.host);  // localhost
print(config.database.port);  // 5432
print(config.features[0]);    // auth
```

### `stringify(value: Any): String`

Converts a Pleno value (Null, Bool, Int, Float, String, Array or Map) to a YAML string. Other values, such as functions, throw an error.

```pleno
import { YAML } from "std:yaml";

let data = {
    "name": "Pleno",
    "version": "0.1.0",
    "features": ["fast", "safe"]
};

print(YAML.stringify(data));
// name: Pleno
// version: 0.1.0
// features:
// - fast
// - safe
```

---
