---
layout: page
footer: false
title: "Testing"
code: |
  import { describe, test, assert } from "std:test";

  fn slugify(title: String): String {
      return title.replace(" ", "-");
  }

  describe("slugify", fn() {
      test("joins words with dashes", fn() {
          assert.equal(slugify("hello world"), "hello-world");
      });

      test("uses lowercase", fn() {
          assert.equal(slugify("Hello World"), "hello-world");
      });
  });
solution: |
  import { describe, test, assert } from "std:test";

  fn slugify(title: String): String {
      return title.toLowerCase().replace(" ", "-");
  }

  describe("slugify", fn() {
      test("joins words with dashes", fn() {
          assert.equal(slugify("hello world"), "hello-world");
      });

      test("uses lowercase", fn() {
          assert.equal(slugify("Hello World"), "hello-world");
      });
  });
---

<TutorialStep>

# Testing

`std:test` has `describe`, `test` and `assert`. A failing assertion marks that test as failed, and the other tests still run:

```pleno
import { describe, test, assert } from "std:test";

describe("math", fn() {
    test("adds", fn() {
        assert.equal(1 + 1, 2);
    });
});
```

On your computer, `pleno test` runs every test file in a folder and fails if any test does, so it works in CI.

## Your turn

One test fails. Run the code, read the message, and fix `slugify` so both tests pass.

</TutorialStep>
