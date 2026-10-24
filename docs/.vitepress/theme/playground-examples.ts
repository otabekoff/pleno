// Examples offered in the playground. Each runs as written.
export const examples: { name: string; code: string }[] = [
  {
    name: 'Hello',
    code: `fn greet(name: String): String {
    return \`Hello, \${name}!\`;
}

print(greet("Pleno"));
`,
  },
  {
    name: 'Classes',
    code: `class Account {
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
print(\`Balance: \${account.total()}\`);

try {
    account.deposit(-5.0);
} catch (e) {
    print("Refused:", e.message);
}
`,
  },
  {
    name: 'Pattern matching',
    code: `enum Shape {
    Circle(radius),
    Rect(width, height)
}

fn area(shape: Shape): Float {
    match (shape) {
        case Shape.Circle(r) => return 3.14159 * r * r,
        case Shape.Rect(w, h) => return w * h
    }
}

let shapes = [Shape.Circle(2.0), Shape.Rect(3.0, 4.5)];
for (shape in shapes) {
    print(\`\${shape}: \${area(shape)}\`);
}
`,
  },
  {
    name: 'Pipelines',
    code: `let squares = 1..=10
    |> filter(fn(n) => n % 2 == 0)
    |> map(fn(n) => n * n);

print(squares);
print(squares |> reduce(fn(sum, n) => sum + n, 0));
`,
  },
  {
    name: 'Async and channels',
    code: `import { Channel } from "std:concurrency";

let jobs = Channel(4);

spawn(fn() {
    for (i in 1..=5) {
        jobs.send(\`job \${i}\`);
    }
    jobs.close();
});

for (job in jobs) {
    print("handled", job);
}
`,
  },
  {
    name: 'Types',
    code: `fn average(values: Float[]): Float {
    let total = values.reduce(fn(a, b) => a + b, 0.0);
    return total / values.length;
}

print(average([3.0, 4.5, 6.0]));

// Annotations are checked when the code runs:
print(average("not a list"));
`,
  },
  {
    name: 'JSON',
    code: `import { JSON } from "std:json";

let user = JSON.parse(\`{"name": "Ada", "langs": ["Pleno", "Rust"]}\`);
print(user.name, "knows", user.langs.length, "languages");

user["active"] = true;
print(JSON.stringifyPretty(user));
`,
  },
  {
    name: 'Tests',
    code: `import { describe, test, assert } from "std:test";

fn slugify(title: String): String {
    return title.toLowerCase().replace(" ", "-");
}

describe("slugify", fn() {
    test("joins words", fn() {
        assert.equal(slugify("Hello World"), "hello-world");
    });

    test("keeps one word", fn() {
        assert.equal(slugify("Pleno"), "pleno");
    });
});
`,
  },
]
