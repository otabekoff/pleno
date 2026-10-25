# Write tests

Tests use the `std:test` module. Type `describe` or `test` for a snippet.

```pleno
import { describe, test, assert } from "std:test";

describe("math", fn() {
    test("adds numbers", fn() {
        assert.equal(1 + 2, 3);
    });
});
```

Each `test(...)` appears in the **Testing** view, where you can run a file, a group or all of
them and see which tests failed and why. In a terminal, `plm test` runs every file in `tests/`,
and `pleno test <file>` runs one.
