---
layout: home

hero:
  name: "Pleno"
  text: "Dynamic code, checked types"
  tagline: "A readable language with optional type annotations, classes and pattern matching, async built in, and the tools to go with it."
  image:
    src: /logo.svg
    alt: Pleno logo
  actions:
    - theme: brand
      text: Get started
      link: /guide/getting-started
    - theme: alt
      text: Why Pleno
      link: /guide/introduction
    - theme: alt
      text: GitHub
      link: https://github.com/otabekoff/pleno

features:
  - icon: 🧭
    title: Types when you want them
    details: "Write <code>let x = 5</code>, or annotate parameters, fields and return values. Annotations are checked when the code runs, including generics, unions, Result and Option."
    link: /specs/core-syntax-fundamentals
    linkText: Types and syntax
  - icon: 🧩
    title: Classes, enums and patterns
    details: Classes with real private fields, interfaces, enums that carry data and methods, and match expressions over all of them.
    link: /specs/OOP
    linkText: Object-oriented Pleno
  - icon: ⚡
    title: Async built in
    details: async and await, promises, spawn, channels and mutexes, with an event loop in the runtime.
    link: /specs/async-concurrency
    linkText: Async and concurrency
  - icon: 📦
    title: A useful standard library
    details: Files, processes, an HTTP client and server, JSON, YAML, TOML, CSV, regular expressions, dates, crypto, SQLite and testing.
    link: /stdlib/
    linkText: Standard library
  - icon: 🛠️
    title: Tools included
    details: Run, check, format, lint and test with pleno; manage packages with plm; edit with the language server, debugger and VS Code extension.
    link: /guide/editor
    linkText: Editor setup
  - icon: 🛡️
    title: A safe runtime
    details: A bytecode VM written in Rust, with limits on memory, instructions, file and network access for running untrusted code.
    link: /specs/security-features
    linkText: Security
---

<div class="home-showcase">

## Pleno at a glance

Each example runs as shown with `pleno run`.

::: code-group

```pleno [Classes]
class Account {
    private balance: Float = 0.0;

    fn deposit(amount: Float) {
        if (amount <= 0.0) {
            throw Error("deposit must be positive");
        }
        this.balance += amount;
    }

    fn total(): Float {
        return this.balance;
    }
}

let account = Account();
account.deposit(25.5);
print(`Balance: ${account.total()}`);   // Balance: 25.5
```

```pleno [Pattern matching]
enum Shape {
    Circle(radius),
    Rect(width, height)
}

fn area(shape: Shape): Float {
    match (shape) {
        case Shape.Circle(r) => return 3.14159 * r * r,
        case Shape.Rect(w, h) => return w * h
    }
}

let shapes = [Shape.Circle(1.0), Shape.Rect(2.0, 3.0)];
print(shapes |> map(fn(s) => area(s)));   // [3.14159, 6]
```

```pleno [Async]
import { Channel } from "std:concurrency";

let jobs = Channel(10);

spawn(fn() {
    for (i in 1..=3) {
        jobs.send(i * 10);
    }
    jobs.close();
});

let mut total = 0;
for (job in jobs) {
    total += job;
}
print(total);   // 60
```

```pleno [Tests]
import { describe, test, assert } from "std:test";

fn slugify(title: String): String {
    return title.toLowerCase().replace(" ", "-");
}

describe("slugify", fn() {
    test("joins words", fn() {
        assert.equal(slugify("Hello World"), "hello-world");
    });
});
```

:::

</div>
