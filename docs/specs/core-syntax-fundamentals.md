# Core Syntax & Fundamentals

### 1.1 Comments
```pleno
// Single-line comment

/* Multi-line comment
   spans multiple lines */

/// Documentation comment for functions, classes, modules
/// `pleno doc` turns these into HTML
```

### 1.2 Variables & Constants

#### Variables
```pleno
let x = 10;                    // Type inference
let name: String = "Alice";    // Explicit type
let mut counter = 0;           // Explicitly mutable
```

A binding without `mut` cannot be reassigned from anywhere, including from a function or closure that captures it: `let x = 1; fn f() { x = 2; }` is a compile error (`cannot assign to immutable binding`). The same holds for `++` and `--`. Names declared with `fn`, `class`, `enum` and `interface` cannot be assigned either (`cannot assign to function 'f'`); use `let mut` for a name that should point to different values over time.

#### Destructuring
```pleno
let (name, age) = ("Alice", 30);       // Tuple
let [first, ...rest] = [1, 2, 3];      // Array, with a rest element
let mut (x, _) = (1, 2);               // `_` skips a value
print(name, age, first, rest, x);      // Alice 30 1 [2, 3] 1
```

See [Destructuring](./data-structures#destructuring) for the rules.

#### Constants (truly immutable)
```pleno
const PI = 3.14159;            // Compile-time constant
const MAX_USERS: Int = 1000;   // Typed constant

const list = [1, 2, 3];
// list = [4, 5, 6];  // compile error: cannot assign to immutable binding
// list.push(4);      // runtime error: cannot modify immutable array
```

A `const` cannot be reassigned (a compile error), and the array, map or object it holds cannot be changed (a runtime error). Values nested inside it, such as an array stored in a `const` map, can still change; `const deep` freezes those too.

#### Deep Immutability
`const deep` freezes the value and everything reachable from it. Changing any part throws `cannot modify immutable map` (or `array`).

```pleno
const deep obj = {
    name: "Alice",
    scores: [95, 87, 92]
};
// obj.name = "Bob";           // ERROR
// obj.scores.push(88);         // ERROR
// obj.scores[0] = 100;         // ERROR
```

### 1.3 Type System

#### Primitive Types
```text
Int      // 64-bit signed integer
Float    // 64-bit floating point
String   // UTF-8 string
Bool     // true or false
Char     // Single Unicode character
Null     // null type
Void     // No return value
Any      // Dynamic type (opt-out of type checking)
```

Ints and Floats compare by value: `5.0 == 5` is `true` and `5.5 == 5` is `false`. The same holds in sets (`Set([5, 5.0])` has one element), `contains`, `indexOf` and `assert.equal`.

```pleno
print(10 / 2 == 5);           // true
print(5.5 != 5);              // true
print(Set([5, 5.0]).size());  // 1
```

Arrays, tuples, maps and sets compare by contents. Arrays and tuples are equal when they have the same elements in the same order (a named tuple's names must match too), maps when they have the same keys with equal values in any order, and sets when they have the same elements. Nested values compare the same way, and values that contain themselves compare without looping. Class instances are equal only to themselves, unless their class defines `operator ==` (see [Operator overloading](./advanced-features.md)), which is also used for instances inside arrays, tuples and maps. `contains` and `indexOf` use the same rules.

```pleno
print([1, [2, 3]] == [1, [2, 3]]);       // true
print({"a": 1, "b": 2} == {"b": 2, "a": 1}); // true
print((1, "x") == (1, "x"));             // true
print([1, 2] == [2, 1]);                 // false
print([[1]].contains([1]));              // true
```

A `Char` and a one-character `String` are different values: `'a' == "a"` is `false`. Indexing a string gives a `String` (`"ab"[0]` is `"a"`).

#### Numbers

Int literals are written in decimal, hexadecimal (`0xff`), binary (`0b101`) or octal (`0o17`), and may use `_` between digits (`1_000_000`). A literal with a `.` or an exponent (`2.5`, `1e3`) is a `Float`.

- **Overflow.** `Int` arithmetic that leaves the 64-bit range throws `integer overflow in addition` (or `subtraction`, `multiplication`, `negation`) instead of wrapping. `Float` arithmetic does not throw: it can reach infinity (`1e308 * 10`) or NaN (`Math.sqrt(-1)`), and NaN is not equal to anything, itself included.
- **Division.** `/` always gives a `Float`, also for two Ints: `7 / 2` is `3.5` and `6 / 2` is `3.0`. For a whole-number quotient, use `Math.floor(a / b)` (or `Math.trunc`), which returns an `Int`. There is no `**` operator; `Math.pow` returns a `Float`.
- **Remainder.** `%` gives the remainder with the sign of the left operand, as in C, Java, JavaScript and Rust (not Python): `7 % 3` is `1`, `-7 % 3` is `-1` and `7 % -3` is `1`. It works on Floats too: `-7.5 % 2` is `-1.5`.
- **Zero.** Dividing by zero, or taking a remainder by zero, throws `division by zero` (or `modulo by zero`), for Floats as well as Ints: `1.0 / 0.0` throws too.
- **Mixing.** An operation on an `Int` and a `Float` gives a `Float`: `2 * 1.5` is `3.0`.

```pleno
print(7 / 2, 6 / 2, -7 % 3, 2 * 1.5);   // 3.5 3.0 -1 3.0

try {
    let big = 9223372036854775807;
    print(big + 1);
} catch (e) {
    print(e.message);                    // integer overflow in addition
}

try {
    let zero = 0.0;
    print(1.0 / zero);
} catch (e) {
    print(e.message);                    // division by zero
}
```

#### String literals

- `"..."` is a string. It can span several lines.
- `` `...` `` is a template string: `${expression}` inside it is replaced by the value of the expression, printed the way `print` shows it.
- `r"..."` is a raw string: a backslash in it is an ordinary character, which suits regular expressions (`r"\d+"`) and Windows paths.
- `'a'` is a `Char`, one character.

In `"..."`, template strings and characters, a backslash starts an escape:

| Escape | Character |
| --- | --- |
| `\n`, `\r`, `\t` | newline, carriage return, tab |
| `\0` | the NUL character |
| `\\`, `\"`, `\'`, `` \` `` | the character itself |
| `\$` | a `$` that does not start `${` |
| `\u{e9}` | the Unicode character with that hex code (1 to 6 digits) |

Any other escape, such as `"\d"` or `"C:\new\x"`, is a compile error (`unknown escape '\d'`) instead of a silent change of meaning: write `\\` for a backslash, or use a raw string.

```pleno
print("tab:\tend", "caf\u{e9}", r"C:\new\x");   // tab:	end café C:\new\x
print(`2 + 2 = ${2 + 2}, not \${x}`);           // 2 + 2 = 4, not ${x}
```

#### Type Annotations
```pleno
let age: Int = 25;
let price: Float = 99.99;
let active: Bool = true;
let data: Any = "flexible";

// Function with type annotations
fn calculate(x: Int, y: Float): Float {
    return x * y;
}
```

Annotations are checked when the program runs: on parameters, `let` bindings, assignments to annotated variables, class fields and return values. A value of the wrong type throws a `TypeError`:

```pleno
fn label(count: Int): String {
    return count;   // TypeError: label returned Int, expected String
}
```

A return annotation is checked at every `return`, and also when a function ends without one: it then returns `null`, which an `Int` annotation rejects (`TypeError: f returned Null, expected Int`) and `Int?` accepts. For an `async fn` the annotation describes the value the promise resolves to, so `async fn f(): Int` and `async fn f(): Promise<Int>` both check for an `Int`. Not checked: `Void` and `Any` returns, returns whose annotation mentions a type parameter (`T`, `T[]`), the type arguments of a user generic class (`Box<Int>` checks for a `Box`), setters, constructors and functions marked `@inline`. The check runs at the `return` statement, so a `try`/`catch` around that `return` in the same function catches its `TypeError`.

**Int and Float.** An `Int` is accepted where a `Float` is expected, and it becomes a `Float` there: in `fn half(x: Float)`, `half(7)` binds `x` to `7.0`, so `x / 2` is `3.5`. The same applies to `let`, assignments, fields and return values, and to `Float?` and unions such as `Float | String`. In a union that also allows `Int` (`Int | Float`, `Number`, `Any`), an `Int` stays an `Int`. Collections are checked but not copied: a `Float[]` (or `Map<String, Float>`) accepts `[1, 2.5]` and keeps the `1` an `Int`. The conversion shows when the value is printed: a `Float` always prints as one, so `let ratio: Float = 1; print(ratio);` prints `1.0`.

```pleno
fn half(x: Float): Float {
    return x / 2;
}
print(half(7));             // 3.5
let ratio: Float = 1;
print(ratio);               // 1.0
print(typeof(ratio));       // Float
let either: Int | Float = 1;
print(typeof(either));      // Int
```

#### Type Inference
```pleno
let count = 42;              // Inferred as Int
let ratio = 3.14;            // Inferred as Float
let message = "Hello";       // Inferred as String
let mut mixed = null;        // Null for now; `mut` allows another value later
```

#### Union Types
```pleno
let mut result: Int | String = 42;
result = "error";  // Valid

fn process(value: Int | Float | String): String {
    return value.toString();
}
```

#### Optional Types
```pleno
let optional: Int? = null;   // Can be Int or null
let username: String? = "alice";

// Safe navigation operator
let length = username?.length;  // Returns Int? (null if username is null)

// Null coalescing
let displayName = username ?? "Guest";

// Forced unwrap (runtime error if null)
let definite = username!;  // Use with caution
```

#### Type Guards
```pleno
fn processValue(val: Int | String) {
    if (val is Int) {
        // val is narrowed to Int here
        print(val * 2);
    } else {
        // val is narrowed to String here
        print(val.toUpperCase());
    }
}
```


### 1.4 TypeOf Operator

`typeof(value)` returns the name of the value's type as a string: `"Int"`, `"Float"`, `"String"`, `"Char"`, `"Bool"`, `"Null"`, `"Array"`, `"Map"`, `"Set"`, `"Tuple"` or `"Range"`; the class name for an instance (`"Person"`, `"Error"`); `"Function"` for every function, lambda, method and built-in function; `"Class"` for a class itself; `"Variant"` for an enum value and `"Enum"` for an enum. `Ok`, `Err`, `Some` and `None` are maps underneath, so `typeof(Ok(1))` is `"Map"`; ask them with `isOk()` or `isSome()` instead.

```pleno
class Person {}
let x = 42;
print(typeof(x));            // Int
print(typeof(Person()));     // Person
print(typeof(fn() => 1));    // Function
print(typeof([1, 2]));       // Array
```

### 1.5 How values print

`print`, string concatenation (`"n = " + n`), template strings and `toString()` all turn a value into text the same way:

| Value | Prints as |
| --- | --- |
| `Int` | its digits: `42`, `-7` |
| `Float` | always as a Float: `3.0`, `2.5`, `0.30000000000000004`; the shortest digits that read back as the same number, with an exponent when very large or small (`1e21`, `1e-7`); `inf`, `-inf` and `NaN` |
| `String`, `Char` | the text itself, without quotes |
| `Bool`, `null` | `true`, `false`, `null` |
| `Array` | `[1, "two", 'c', null]` |
| `Map` | `{"name": "Ada", "age": 36}`, in insertion order |
| `Set` | `{1, 2, 3}` |
| `Tuple` | `(1, "a")`, `(x: 1, y: 2)`, and `(1,)` for one element |
| `Ok(1)`, `Err("x")`, `Some(2)`, `None` | as written |
| enum value | `Color.Red`, `Shape.Circle(2.0)` |
| `Error` | `name: message`, such as `TypeError: expected Int, found String` |
| `Range` | `0..3`, `0..=3` |
| instance of a class | `<instance of Person>`; a `toString` method is not called, so call it yourself |
| class, function | `<class Person>`, `<fn>` and similar; the exact text is not specified |

Inside an array, map, set, tuple or enum value, strings and characters are quoted and escaped the way they are written in source (`"a\"b"`, `'\n'`), so `["1", 1]` and `[1, 1]` print differently. A container that contains itself prints `[...]` (or `{...}`, `(...)`) at the point where it repeats, and nesting deeper than 200 levels is cut off the same way.

```pleno
print("text", ["text", 1, "1"], {"k": "v"}, ("a",));   // text ["text", 1, "1"] {"k": "v"} ("a",)
print(10 / 4, 10 / 5, 1e21);                            // 2.5 2.0 1e21
print(Ok(1), None);                                     // Ok(1) None

let items = [1];
items.push(items);
print(items);                                           // [1, [...]]
```

### 1.6 Scope and shadowing

A `let` or `const` belongs to the block (`{ ... }`) it is declared in, from that point to the end of the block. A function body, a loop body and each branch of an `if` are blocks.

- An inner block can declare a name that already exists outside it. The inner name hides the outer one until the block ends; the outer variable is not changed.
- Declaring the same name twice in the same block is a compile error (`Local 'x' already defined`). Use `let mut` and assign instead.
- A parameter can be redeclared with `let` inside its function, which hides the parameter.

```pleno
let x = 1;
{
    let x = 2;
    print(x);    // 2
}
print(x);        // 1

fn next(n: Int): Int {
    let n = n + 1;
    return n;
}
print(next(1));  // 2
```

### 1.7 Reserved words

These words are keywords and cannot be used as names:
`abstract`, `as`, `async`, `await`, `break`, `case`, `catch`, `class`, `const`, `continue`, `deep`, `do`, `else`, `enum`, `export`, `extend`, `extends`, `false`, `finally`, `fn`, `for`, `from`, `if`, `implements`, `import`, `in`, `interface`, `internal`, `is`, `lazy`, `let`, `macro`, `match`, `module`, `move`, `mut`, `new`, `null`, `operator`, `private`, `protected`, `public`, `return`, `static`, `super`, `this`, `throw`, `true`, `try`, `type`, `while` and `_`.

`get`, `set`, `out` and `property` are keywords in some positions (property accessors, variance annotations) and are accepted as variable names elsewhere, but not, for example, as names in an `import { ... }` list. Avoid them as names.