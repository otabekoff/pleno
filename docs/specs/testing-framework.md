# Testing Framework

Pleno includes a built-in testing framework in `std:test`.

## Quick Start

```pleno
import { describe, test, assert } from "std:test";

describe("Math operations", fn() {
    test("addition works", fn() {
        assert.equal(1 + 1, 2);
    });

    test("multiplication works", fn() {
        assert.equal(3 * 4, 12);
    });
});
```

Run with:

```bash
pleno test tests/
```

## Test Structure

### Test Suites

Group related tests with `describe`:

```pleno
describe("Calculator", fn() {
    describe("add", fn() {
        test("positive numbers", fn() { ... });
        test("negative numbers", fn() { ... });
    });

    describe("divide", fn() {
        test("whole numbers", fn() { ... });
        test("by zero throws", fn() { ... });
    });
});
```

### Lifecycle Hooks

```pleno
describe("Database", fn() {
    let db;

    beforeEach(fn() {
        db = Database.connect("test");
    });

    afterEach(fn() {
        db.close();
    });

    test("insert works", fn() {
        db.insert({ "name": "Alice" });
        assert.equal(db.count(), 1);
    });
});
```

## Assertions

| Assertion                     | Description   |
| ----------------------------- | ------------- |
| `assert.equal(a, b)`          | Deep equality |
| `assert.notEqual(a, b)`       | Not equal     |
| `assert.true(val)`            | Is true       |
| `assert.false(val)`           | Is false      |
| `assert.null(val)`            | Is null       |
| `assert.notNull(val)`         | Not null      |
| `assert.throws(fn)`           | Throws error  |
| `assert.contains(coll, item)` | Contains item |
| `assert.lengthOf(coll, n)`    | Has length n  |

### Examples

```pleno
// Equality
assert.equal([1, 2, 3], [1, 2, 3]);
assert.equal({ "a": 1 }, { "a": 1 });

// Boolean
assert.true(5 > 3);
assert.false(2 > 5);

// Null checks
assert.null(null);
assert.notNull("hello");

// Exceptions
assert.throws(fn() {
    throw Error("boom");
});

// Collections
assert.contains([1, 2, 3], 2);
assert.contains("hello", "ell");
assert.lengthOf([1, 2, 3], 3);
```

## Async Tests

```pleno
describe("API", fn() {
    test("fetch works", async fn() {
        let data = await fetchData("https://api.example.com");
        assert.notNull(data);
    });
});
```

## Test Organization

```
project/
├── src/
│   └── main.pln
├── tests/
│   ├── unit/
│   │   ├── math_test.pln
│   │   └── string_test.pln
│   └── integration/
│       └── api_test.pln
└── pleno.toml
```

## CLI Options

```bash
# Run all tests
pleno test tests/

# Run specific file
pleno test tests/unit/math_test.pln

# Run with pattern
pleno test tests/ --filter "Calculator"

# Coverage report
pleno test --coverage tests/

# Verbose output
pleno test --verbose tests/
```

## Output

```
Calculator
  add
    ✓ positive numbers
    ✓ negative numbers
  divide
    ✓ whole numbers
    ✓ by zero throws

4 passing (15ms)
```
