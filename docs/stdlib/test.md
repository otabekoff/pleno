# std:test

Built-in testing framework for unit and integration tests.

## Import

```pleno
import { describe, test, beforeEach, afterEach, assert } from "std:test";
```

## Test Structure

### `describe(name: String, fn: Function): Void`

Creates a test suite that groups related tests.

```pleno
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

Defines an individual test case.

```pleno
test("strings concatenate", fn() {
    assert.equal("hello" + " world", "hello world");
});
```

## Lifecycle Hooks

### `beforeEach(fn: Function): Void`

Runs before each test in the current suite.

```pleno
describe("Database", fn() {
    beforeEach(fn() {
        // Setup test database
    });

    test("insert works", fn() { /* ... */ });
    test("delete works", fn() { /* ... */ });
});
```

### `afterEach(fn: Function): Void`

Runs after each test in the current suite.

```pleno
afterEach(fn() {
    // Cleanup resources
});
```

---

## Assertions

### `assert.equal(actual, expected)`

Asserts deep equality.

```pleno
assert.equal([1, 2], [1, 2]);  // ✓
assert.equal(42, 42);          // ✓
```

### `assert.notEqual(a, b)`

Asserts values are not equal.

```pleno
assert.notEqual(1, 2);  // ✓
```

### `assert.true(value)`

Asserts value is `true`.

```pleno
assert.true(5 > 3);  // ✓
```

### `assert.false(value)`

Asserts value is `false`.

```pleno
assert.false(3 > 5);  // ✓
```

### `assert.null(value)`

Asserts value is `null`.

```pleno
assert.null(null);  // ✓
```

### `assert.notNull(value)`

Asserts value is not `null`.

```pleno
assert.notNull("hello");  // ✓
```

### `assert.throws(fn: Function)`

Asserts that the function throws an error.

```pleno
assert.throws(fn() {
    throw Error("expected");
});  // ✓
```

### `assert.contains(collection, item)`

Asserts collection contains item.

```pleno
assert.contains([1, 2, 3], 2);          // ✓
assert.contains("hello", "ell");         // ✓
assert.contains({ "a": 1 }, "a");        // ✓
```

### `assert.lengthOf(collection, length: Int)`

Asserts collection has specific length.

```pleno
assert.lengthOf([1, 2, 3], 3);   // ✓
assert.lengthOf("hello", 5);     // ✓
```

---

## Running Tests

```bash
pleno test tests/            # Run all tests in directory
pleno test my_test.pln       # Run specific test file
```

---
