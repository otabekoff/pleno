---
layout: page
footer: false
title: "Strings"
code: |
  let title = "Pleno in practice";

  // Print the title in capitals and its length.
solution: |
  let title = "Pleno in practice";

  print(`${title.toUpperCase()} (${title.length} characters)`);
---

<TutorialStep>

# Strings

Strings use double quotes. Backticks make a template, where `${...}` inserts the value of any expression, and which may span several lines:

```pleno
let name = "Ada";
print(`Hello, ${name}! You have ${2 + 3} messages.`);
```

Strings have methods: `toUpperCase()`, `toLowerCase()`, `trim()`, `split(sep)`, `replace(from, to)`, `contains(text)`, `startsWith(text)`, and the `length` property.

## Your turn

Print the title in capitals, followed by how many characters it has, like `PLENO IN PRACTICE (17 characters)`.

</TutorialStep>
