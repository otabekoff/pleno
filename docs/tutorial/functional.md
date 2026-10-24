---
layout: page
footer: false
title: "Functions as values"
code: |
  // Expected: [1, 9, 25, 49, 81]
  let squares = 1..=9;

  print(squares);
solution: |
  // Expected: [1, 9, 25, 49, 81]
  let squares = 1..=9
      |> filter(fn(n) => n % 2 == 1)
      |> map(fn(n) => n * n);

  print(squares);
---

<TutorialStep>

# Functions as values

`map`, `filter` and `reduce` work on arrays and ranges, and the pipeline operator `|>` passes a value into the next call as its first argument, so data reads left to right:

```pleno
let total = [5, 12, 8, 20]
    |> filter(fn(n) => n > 6)
    |> map(fn(n) => n * 2)
    |> reduce(fn(sum, n) => sum + n, 0);

print(total);   // 80
```

`_` leaves a gap to fill later: `let double = multiply(_, 2);` makes a function that takes the missing argument.

## Your turn

Use a pipeline to print the squares of the odd numbers from 1 to 9.

</TutorialStep>
