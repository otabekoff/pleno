# Testing

Tests use the `std:test` module and run with `pleno test` or `plm test`.

```pleno
import { describe, test, assert } from "std:test";

fn slugify(title: String): String {
    return title.toLowerCase().replace(" ", "-");
}

describe("slugify", fn() {
    test("lowercases and joins words", fn() {
        assert.equal(slugify("Hello World"), "hello-world");
    });

    test("keeps a single word", fn() {
        assert.equal(slugify("Pleno"), "pleno");
    });
});
```

```sh
pleno test tests/        # every .pln file in tests/
plm test                 # the tests/ folder of the current project
```

Each file passes if all its tests pass and it runs without an error. A failing test is reported with its message, the other tests still run, and the command exits with a non-zero code, so CI notices.

Files that declare `module Name;` are libraries for other files and are skipped.

## Assertions

| Assertion | Passes when |
| --- | --- |
| `assert.equal(a, b)` | `a` and `b` are equal, comparing arrays and maps by content |
| `assert.notEqual(a, b)` | they are not equal |
| `assert.true(value)`, `assert.false(value)` | `value` is `true` or `false` |
| `assert.null(value)`, `assert.notNull(value)` | `value` is or is not `null` |
| `assert.throws(fn)` | calling `fn` throws |
| `assert.contains(collection, item)` | the array, string or map contains `item` |
| `assert.lengthOf(collection, n)` | it has `n` elements |

`beforeEach` and `afterEach` run a function around every test in a `describe` block. The [std:test reference](/stdlib/test) has the details.
