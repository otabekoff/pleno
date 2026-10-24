---
layout: page
footer: false
title: "Functions"
code: |
  fn makeCounter() {
      // Keep a count and return a function that adds one and returns it.
  }

  let next = makeCounter();
  print(next(), next(), next());   // 1 2 3
solution: |
  fn makeCounter() {
      let mut count = 0;
      return fn() {
          count += 1;
          return count;
      };
  }

  let next = makeCounter();
  print(next(), next(), next());   // 1 2 3
---

<TutorialStep>

# Functions

`fn` declares a function. Parameters can have default values, and callers can name arguments:

```pleno
fn greet(name, greeting = "Hello") {
    return `${greeting}, ${name}!`;
}

print(greet("Ada"));
print(greet("Alan", greeting: "Hi"));
```

A function without a name is a **lambda**: `fn(x) => x * 2` for one expression, or `fn(x) { ... }` with a body. Functions are values you can store and pass around, and they remember the variables around them (closures).

## Your turn

Write `makeCounter()`, which returns a function that counts up by one each time it is called.

</TutorialStep>
