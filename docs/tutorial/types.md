---
layout: page
footer: false
title: "Types"
code: |
  fn average(values: Float[]): Float {
      let total = values.reduce(fn(a, b) => a + b, 0.0);
      return total / values.length;
  }

  print(average([2.0, 4.0, 9.0]));
  print(average("3, 4, 5"));
solution: |
  fn average(values: Float[]): Float {
      let total = values.reduce(fn(a, b) => a + b, 0.0);
      return total / values.length;
  }

  print(average([2.0, 4.0, 9.0]));
  print(average([3.0, 4.0, 5.0]));
---

<TutorialStep>

# Types

Types are optional. When you add them, Pleno checks them as the program runs:

```pleno
fn area(width: Float, height: Float): Float {
    return width * height;
}

let label: String = "room";
let maybe: Int? = null;            // Int or null
let id: Int | String = "user-7";   // either one
let tags: String[] = ["new"];
```

A wrong value stops the program with a clear message instead of failing somewhere later. An `Int` is accepted where a `Float` is expected.

## Your turn

Run the program and read the error. Then fix the call so the program finishes.

</TutorialStep>
