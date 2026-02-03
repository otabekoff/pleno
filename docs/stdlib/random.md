# std:random

Random number generation and selection utilities.

## Import

```pleno
import { Random } from "std:random";
```

## Functions

### `random(): Float`

Returns a random floating-point number between 0.0 (inclusive) and 1.0 (exclusive).

```pleno
let r = Random.random();  // e.g., 0.7234
```

### `randint(min: Int, max: Int): Int`

Returns a random integer between `min` and `max` (inclusive).

```pleno
let dice = Random.randint(1, 6);  // 1, 2, 3, 4, 5, or 6
let coin = Random.randint(0, 1);  // 0 or 1
```

### `choice(array: T[]): T`

Returns a random element from the array.

```pleno
let colors = ["red", "green", "blue"];
let picked = Random.choice(colors);  // e.g., "green"

let empty = [];
Random.choice(empty);  // null
```

### `shuffle(array: T[]): Void`

Shuffles the array in place (mutates the original).

```pleno
let deck = [1, 2, 3, 4, 5];
Random.shuffle(deck);
// deck is now in random order, e.g., [3, 1, 5, 2, 4]
```

> **Note**: `shuffle` modifies the original array. Use on mutable arrays only.

---
