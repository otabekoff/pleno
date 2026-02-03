# std:uuid

UUID generation and parsing.

## Import

```pleno
import { UUID } from "std:uuid";
```

## Functions

### `v4(): String`

Generates a random UUID v4.

```pleno
let id = UUID.v4();
// "550e8400-e29b-41d4-a716-446655440000"
```

### `v7(): String`

Generates a time-ordered UUID v7 (sortable by creation time).

```pleno
let id = UUID.v7();
// "01912345-6789-7abc-8def-0123456789ab"
```

### `parse(uuid: String): String`

Validates and normalizes a UUID string.

```pleno
let normalized = UUID.parse("550E8400E29B41D4A716446655440000");
// "550e8400-e29b-41d4-a716-446655440000"
```

Throws an error if the UUID is invalid.

---
