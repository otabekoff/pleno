# std:test

Built-in testing framework for unit and integration tests.

## Import

```pleno
import { describe, test, beforeEach, afterEach, assert } from "std:test";
```

`assert` here is an object of assertion functions. Without the import, `assert` is the global function `assert(condition, message?)`, which throws when the condition is falsy.

## Test Structure

### `describe(name: String, fn: Function): Void`

Creates a test suite that groups related tests. It prints the suite name and runs `fn` right away. Suites can be nested.

```pleno
import { describe, test, assert } from "std:test";

describe("Calculator", fn() {
    test("addition works", fn() {
        assert.equal(1 + 1, 2);
    });

    test("subtraction works", fn() {
        assert.equal(5 - 3, 2);
    });
});
```

### `test(name: String, fn: Function): Void`

Defines and runs one test case. A passing test prints `✓ name`. A failing test prints `✗ name` and the error message. The other tests still run, and the file fails at the end.

```pleno
import { test, assert } from "std:test";

test("strings concatenate", fn() {
    assert.equal("hello" + " world", "hello world");
});
```

## Lifecycle Hooks

### `beforeEach(fn: Function): Void`

Runs before each test in the current suite, including tests in nested suites. Call it inside `describe`.

```pleno
import { describe, test, beforeEach, assert } from "std:test";

describe("Counter", fn() {
    let mut items = [];

    beforeEach(fn() {
        items = [];
    });

    test("starts empty", fn() {
        assert.lengthOf(items, 0);
    });

    test("push adds an item", fn() {
        items.push(1);
        assert.lengthOf(items, 1);
    });
});
```

### `afterEach(fn: Function): Void`

Runs after each test in the current suite. Call it inside `describe`.

```pleno
import { describe, test, afterEach, assert } from "std:test";

describe("Cleanup", fn() {
    afterEach(fn() {
        print("cleaned up");
    });

    test("runs first", fn() {
        assert.isTrue(true);
    });
});
```

---

## Assertions

Each assertion throws an error when it fails.

### `assert.equal(actual, expected)`

Asserts deep equality. Arrays and maps are compared element by element.

```pleno
import { assert } from "std:test";

assert.equal([1, 2], [1, 2]);
assert.equal({ "a": [1] }, { "a": [1] });
assert.equal(42, 42);
```

### `assert.notEqual(a, b)`

Asserts values are not equal.

```pleno
import { assert } from "std:test";

assert.notEqual(1, 2);
```

### `assert.isTrue(value)`

Asserts value is `true`. Alias: `assert.true`.

```pleno
import { assert } from "std:test";

assert.isTrue(5 > 3);
```

### `assert.isFalse(value)`

Asserts value is `false`. Alias: `assert.false`.

```pleno
import { assert } from "std:test";

assert.isFalse(3 > 5);
```

### `assert.isNull(value)`

Asserts value is `null`. Alias: `assert.null`.

```pleno
import { assert } from "std:test";

assert.isNull(null);
```

### `assert.notNull(value)`

Asserts value is not `null`.

```pleno
import { assert } from "std:test";

assert.notNull("hello");
```

### `assert.throws(fn: Function)`

Asserts that the function throws an error.

```pleno
import { assert } from "std:test";

assert.throws(fn() {
    throw Error("expected");
});
```

### `assert.contains(collection, item)`

Asserts that a string contains a substring, an array contains an item, or a map has a key.

```pleno
import { assert } from "std:test";

assert.contains([1, 2, 3], 2);
assert.contains("hello", "ell");
assert.contains({ "a": 1 }, "a");
```

### `assert.lengthOf(collection, length: Int)`

Asserts that a string, array, or map has a specific length. For strings, the length is in bytes.

```pleno
import { assert } from "std:test";

assert.lengthOf([1, 2, 3], 3);
assert.lengthOf("hello", 5);
assert.lengthOf({ "a": 1 }, 1);
```

---

## Running Tests

```bash
pleno test                   # Run all .pln files in tests/
pleno test tests/            # Run all tests in a directory
pleno test my_test.pln       # Run a specific test file
```
