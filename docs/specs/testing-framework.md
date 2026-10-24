# Testing Framework

Pleno includes a built-in testing framework in `std:test`. See [std:test](/stdlib/test) for the full API.

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

`describe` prints the suite name and runs its function right away. `test` runs one test and prints `✓ name` or `✗ name` with the error. A test file is an ordinary program, so `pleno run` works on it too. Run all test files with:

```bash
pleno test tests/
```

## Test Structure

### Test Suites

Group related tests with `describe`. Suites can be nested.

```pleno
import { describe, test, assert } from "std:test";

describe("Calculator", fn() {
    describe("add", fn() {
        test("positive numbers", fn() {
            assert.equal(2 + 3, 5);
        });
        test("negative numbers", fn() {
            assert.equal(-2 + -3, -5);
        });
    });

    describe("divide", fn() {
        test("fractions", fn() {
            assert.equal(10 / 4, 2.5);
        });
        test("by zero throws", fn() {
            assert.throws(fn() {
                let x = 1 / 0;
            });
        });
    });
});
```

`/` returns a Float, and `assert.equal` does not treat a Float and an Int as equal: `assert.equal(10 / 2, 5)` fails, `assert.equal(10 / 2, 5.0)` passes.

### Lifecycle Hooks

`beforeEach` and `afterEach` run before and after each test in the current suite. Call them inside `describe`.

```pleno
import { describe, test, beforeEach, afterEach, assert } from "std:test";

describe("Store", fn() {
    let mut items = [];

    beforeEach(fn() {
        items = [];
        items.push("seed");
    });

    afterEach(fn() {
        items.clear();
    });

    test("starts with one item", fn() {
        assert.lengthOf(items, 1);
    });

    test("push adds an item", fn() {
        items.push("Alice");
        assert.equal(items, ["seed", "Alice"]);
    });
});
```

## Assertions

Import `assert` from `std:test` to get these functions. Without the import, `assert` is the global function `assert(condition, message?)`.

| Assertion                     | Description                                       |
| ----------------------------- | ------------------------------------------------- |
| `assert.equal(a, b)`          | Deep equality                                     |
| `assert.notEqual(a, b)`       | Not equal                                         |
| `assert.isTrue(val)`          | Is true (alias `assert.true`)                     |
| `assert.isFalse(val)`         | Is false (alias `assert.false`)                   |
| `assert.isNull(val)`          | Is null (alias `assert.null`)                     |
| `assert.notNull(val)`         | Not null                                          |
| `assert.throws(fn)`           | Throws error                                      |
| `assert.contains(coll, item)` | String has substring, array has item, map has key |
| `assert.lengthOf(coll, n)`    | Has length n                                      |

### Examples

```pleno
import { assert } from "std:test";

// Equality
assert.equal([1, 2, 3], [1, 2, 3]);
assert.equal({ "a": 1 }, { "a": 1 });

// Boolean
assert.isTrue(5 > 3);
assert.isFalse(2 > 5);

// Null checks
assert.isNull(null);
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

A test body can be an `async fn`. Its result is reported when it finishes, so it may appear after tests that started later, and a failed assertion after an `await` fails the run like any other:

```pleno
import { test, assert } from "std:test";
import { sleep } from "std:time";

async fn fetchData(name: String): String {
    await sleep(10);
    return "data for " + name;
}

test("fetch works", async fn() {
    assert.equal(await fetchData("users"), "data for users");
});
```

`afterEach` hooks run when the body returns its promise, not when the async work ends.

## Test Organization

`pleno test` with no path runs every `.pln` file in `tests/` (or `sources/tests/`), including subfolders.

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
# Run all tests in tests/
pleno test

# Run all tests in a folder
pleno test tests/

# Run one file
pleno test tests/unit/math_test.pln

# Print the results as JSON
pleno test --json tests/
```

Each file counts as one result. A file fails if any test in it fails or if it throws.

## Output

```
Running tests/calc_test.pln ... Calculator
  add
    ✓ positive numbers
    ✓ negative numbers
  divide
    ✓ fractions
    ✓ by zero throws
PASSED

Test Summary:
  Passed: 1
  Failed: 0
  Total:  1
```
