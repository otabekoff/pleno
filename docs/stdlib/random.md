# std:random

> **Stability:** stable in 1.0.

Random number generation and selection utilities.

## Import

```pleno
import { Random } from "std:random";
```

Single functions can also be imported, e.g. `import { randint } from "std:random";`.

## Functions

### `random(): Float`

Returns a random floating-point number between 0.0 (inclusive) and 1.0 (exclusive).

```pleno
import { Random } from "std:random";

let r = Random.random();  // e.g. 0.3710362675456278
print(r);
```

### `randint(min: Int, max: Int): Int`

Returns a random integer between `min` and `max` (inclusive). `min` must not be greater than `max`.

```pleno
import { Random } from "std:random";

let dice = Random.randint(1, 6);  // 1, 2, 3, 4, 5, or 6
let coin = Random.randint(0, 1);  // 0 or 1
print(dice, coin);
```

### `choice(array: T[]): T?`

Returns a random element from the array, or `null` if the array is empty.

```pleno
import { Random } from "std:random";

let colors = ["red", "green", "blue"];
let picked = Random.choice(colors);  // e.g. "green"
print(picked);

print(Random.choice([]));  // null
```

### `shuffle(array: T[]): Void`

Shuffles the array in place.

```pleno
import { Random } from "std:random";

let deck = [1, 2, 3, 4, 5];
Random.shuffle(deck);
print(deck);  // e.g. [3, 1, 5, 2, 4]
```

> **Note**: `shuffle` changes the original array. It throws on an immutable array.

---
