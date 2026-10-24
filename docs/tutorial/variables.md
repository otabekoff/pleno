---
layout: page
footer: false
title: "Values and variables"
code: |
  let total = 0;
  total += 1;
  total += 2;
  print("total is", total);
solution: |
  let mut total = 0;
  total += 1;
  total += 2;
  print("total is", total);
---

<TutorialStep>

# Values and variables

`let` gives a value a name. A `let` binding cannot be reassigned; `let mut` can. `const` is for values that never change.

```pleno
let language = "Pleno";
let mut visits = 0;
visits += 1;
const MAX_VISITS = 3;
```

The basic values are numbers (`Int` like `42`, `Float` like `3.5`), strings, `true` and `false`, and `null`.

Reassigning a plain `let` is an error, which catches mistakes early. Try adding `language = "Other";` and running it.

## Your turn

`total` should count from 0 up to 3, but the program fails. Fix it.

</TutorialStep>
