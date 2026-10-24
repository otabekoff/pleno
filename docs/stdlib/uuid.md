# std:uuid

UUID generation and parsing.

## Import

```pleno
import { UUID } from "std:uuid";
```

`v4`, `v7` and `parse` can also be imported directly.

## Functions

### `v4(): String`

Generates a random UUID v4 as a lowercase, hyphenated string.

```pleno
import { UUID } from "std:uuid";

let id = UUID.v4();
print(id.length); // 36
// e.g. "550e8400-e29b-41d4-a716-446655440000"
```

### `v7(): String`

Generates a time-ordered UUID v7. Later IDs sort after earlier ones.

```pleno
import { UUID } from "std:uuid";

let id = UUID.v7();
// e.g. "01912345-6789-7abc-8def-0123456789ab"
```

### `parse(uuid: String): String`

Validates a UUID string and returns it in lowercase, hyphenated form.

```pleno
import { UUID } from "std:uuid";

let normalized = UUID.parse("550E8400E29B41D4A716446655440000");
print(normalized); // 550e8400-e29b-41d4-a716-446655440000
```

Throws an error if the UUID is invalid.

---
