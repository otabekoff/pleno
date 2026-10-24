---
layout: page
footer: false
title: "The standard library"
code: |
  import { JSON } from "std:json";

  let text = `[
      {"title": "Dune", "price": 18.5},
      {"title": "Emma", "price": 9.0},
      {"title": "Ulysses", "price": 24.0}
  ]`;

  // Parse it and count the books under 20.
solution: |
  import { JSON } from "std:json";

  let text = `[
      {"title": "Dune", "price": 18.5},
      {"title": "Emma", "price": 9.0},
      {"title": "Ulysses", "price": 24.0}
  ]`;

  let books = JSON.parse(text);
  let cheap = books |> filter(fn(book) => book.price < 20.0);
  print(cheap.length, "books cost less than 20");
---

<TutorialStep>

# The standard library

Modules from the standard library start with `std:`. Import a whole namespace or single names:

```pleno
import { JSON } from "std:json";
import { Math } from "std:math";
import { sqrt } from "std:math";

print(Math.max(3, 7), sqrt(16.0));
```

There are modules for files, paths, HTTP, JSON, YAML, TOML, CSV, regular expressions, dates, cryptography and more; see the [standard library reference](/stdlib/). Your own files are modules too: `export fn` in `utils.pln`, then `import { helper } from "./utils.pln";`.

In the browser, files, the network and processes are not available.

## Your turn

Parse the JSON text and print how many books cost less than 20.

</TutorialStep>
