# std:yaml

YAML parsing and serialization.

## Import

```pleno
import { YAML } from "std:yaml";
```

## Functions

### `parse(yaml: String): Any`

Parses a YAML string into a Pleno value.

```pleno
let config = YAML.parse(`
database:
  host: localhost
  port: 5432

features:
  - auth
  - logging
  - caching
`);

print(config.database.host);  // "localhost"
print(config.features[0]);    // "auth"
```

### `stringify(value: Any): String`

Converts a Pleno value to a YAML string.

```pleno
let data = {
    "name": "Pleno",
    "version": "0.1.0",
    "features": ["fast", "safe", "expressive"]
};

let yaml = YAML.stringify(data);
```

---
