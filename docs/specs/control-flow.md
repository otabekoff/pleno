# Control Flow

## Conditionals

A condition is false only when it is `false` or `null`. Every other value is true, including `0`, `""`, `[]` and `None`. `!`, `&&` and `||` follow the same rule and always produce a `Bool`: `null || 3` is `true`, not `3`; use `??` to pick a default value.

### If and else

```pleno
let temperature = 18;

if (temperature > 25) {
    print("hot");
} else if (temperature > 15) {
    print("pleasant");
} else {
    print("cold");
}

// `if` is also an expression.
let score = 72;
let status = if (score >= 60) "Pass" else "Fail";
print(status);   // Pass
```

### Match

`match` compares a value against patterns from top to bottom and runs the first one that fits. A case can list several values, bind the value to a name, check its type, or add an `if` guard. `_` matches anything.

```pleno
fn describe(value) {
    match (value) {
        case 0 => print("zero"),
        case 1, 2, 3 => print("small"),
        case String s => print("string: " + s),
        case x if x > 100 => print("large: " + x),
        case _ => print("other")
    }
}

describe(2);       // small
describe("hi");    // string: hi
describe(500);     // large: 500
describe(50);      // other

// `match` is also an expression.
let age = 34;
let category = match (age) {
    case x if x < 13 => "child",
    case x if x < 20 => "teen",
    case x if x < 65 => "adult",
    case _ => "senior"
};
print(category);   // adult
```

Enums and their data are matched the same way; see [Data structures](./data-structures).

## Loops

### For

```pleno
// C-style
for (let i = 0; i < 3; i++) {
    print(i);
}

// Over the items of an array, set or range, or the keys of a map
let fruits = ["apple", "pear", "plum"];
for (fruit in fruits) {
    print(fruit);
}

// With the index
for (index, fruit in fruits) {
    print(index + ": " + fruit);
}

// Over a range
for (i in 0..3) {     // 0, 1, 2
    print(i);
}
for (i in 0..=3) {    // 0, 1, 2, 3
    print(i);
}
```

A string cannot be iterated directly (`type 'String' is not iterable`); index it in a loop instead. `for (key, value in map)` gives a map's keys and values.

### While and do-while

```pleno
let mut countdown = 3;
while (countdown > 0) {
    print(countdown);
    countdown -= 1;
}

// The body of a do-while runs at least once.
let mut attempts = 0;
do {
    attempts += 1;
} while (attempts < 3);
print(attempts);   // 3
```

### Break and continue

Inside a loop, `break;` exits it, `continue;` skips to the next iteration, and `break label;` exits a labeled outer loop:

```pleno
for (n in 1..=10) {
    if (n % 2 == 0) continue;
    if (n > 7) break;
    print(n);   // 1, 3, 5, 7
}

outer: for (i in 0..10) {
    for (j in 0..10) {
        if (i * j > 50) {
            print(`stopped at ${i} * ${j}`);   // stopped at 6 * 9
            break outer;
        }
    }
}
```
