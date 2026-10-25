# Data Structures

## Arrays

```pleno
// Array literal
let numbers: Int[] = [1, 2, 3, 4, 5];
let mixed: Any[] = [1, "two", 3.0, true];

// Array operations (they change the array)
numbers.push(6);           // Add to end:      [1, 2, 3, 4, 5, 6]
numbers.pop();             // Remove from end: [1, 2, 3, 4, 5]
numbers.unshift(0);        // Add to start:    [0, 1, 2, 3, 4, 5]
numbers.shift();           // Remove from start
numbers.insert(2, 99);     // Insert at index: [1, 2, 99, 3, 4, 5]
numbers.remove(1);         // Remove at index: [1, 99, 3, 4, 5]

// Array methods
print(numbers.length);       // 5
print(numbers.isEmpty());    // false
print(numbers.contains(3));  // true
print(numbers.indexOf(4));   // 3
numbers.reverse();           // Reverse in place: [5, 4, 3, 99, 1]
let sorted = numbers.sort(); // Returns a sorted copy: [1, 3, 4, 5, 99]

// Functional methods
print(numbers.map(fn(x) => x * 2));               // [10, 8, 6, 198, 2]
print(numbers.filter(fn(x) => x > 3));            // [5, 4, 99]
print(numbers.reduce(fn(acc, x) => acc + x, 0));  // 112
numbers.forEach(fn(x) => print(x));

// Array slicing returns a new array
print(numbers[1..4]);   // [4, 3, 99]
print(numbers[2..]);    // [3, 99, 1]
print(numbers[..3]);    // [5, 4, 3]
print(numbers[1..=3]);  // [4, 3, 99]: `..=` includes the end
print(numbers[..=1]);   // [5, 4]
print(numbers[..=-1]);  // [5, 4, 3, 99, 1]: a negative end counts from the back
```

Strings slice the same way, by character: `"hello"[..=1]` is `"he"`.

## Maps (Dictionaries)

Map keys are strings. In a literal, a key is a string or a bare name (`{host: "x"}` is `{"host": "x"}`); other keys, such as `{1: "a"}`, are a syntax error.

```pleno
// Map literal
let ages: Map<String, Int> = {
    "Alice": 30,
    "Bob": 25,
    "Charlie": 35
};

// Type-inferred map
let scores = {
    "math": 95,
    "science": 88,
    "english": 92
};

// Map operations
ages.put("David", 28);               // Add or update an entry
ages.remove("Bob");                  // Remove an entry
print(ages.get("Alice"));            // 30 (null if the key is missing)
print(ages.containsKey("Charlie"));  // true
print(ages.keys());                  // [Alice, Charlie, David]
print(ages.values());                // [30, 35, 28]
print(ages.size());                  // 3
print(ages.isEmpty());               // false

// Default value if the key is missing
let age = ages["Zed"] ?? 0;          // 0

// Iteration, in insertion order
for (key, value in ages) {
    print(key + ": " + value);       // Alice: 30, Charlie: 35, David: 28
}
```

## Sets

```pleno
let uniqueNumbers: Set<Int> = {1, 2, 3, 4, 5};

// Set operations
uniqueNumbers.add(6);
uniqueNumbers.remove(3);
print(uniqueNumbers);               // {1, 2, 4, 5, 6}
print(uniqueNumbers.contains(4));   // true
print(uniqueNumbers.size());        // 5
print(uniqueNumbers.isEmpty());     // false

// Combining sets
let set1 = {1, 2, 3};
let set2 = {3, 4, 5};

print(set1.union(set2));          // {1, 2, 3, 4, 5}
print(set1.intersection(set2));   // {3}
print(set1.difference(set2));     // {1, 2}
```

`Set(array)` builds a set from an array.

## Tuples

```pleno
// Tuple with different types
let person = ("Alice", 30, true);

// Access by index
print(person.0);  // Alice
print(person.1);  // 30

// Named tuples: fields by name or by index
let point = (x: 10.5, y: 20.3);
print(point.x);   // 10.5
print(point.0);   // 10.5

// A one-element tuple needs a trailing comma
let single = (42,);
print(single.0);  // 42
```

### Tuple types

A tuple type lists the type of each element: `(String, Int, Bool)`. Like other annotations, it is checked when the code runs: the value must be a tuple with exactly that many elements, each of the listed type. Tuple types combine with the other type forms, as in `(Int, Int)[]`, `(Int, String)?` and `Map<String, (Int, Int)>`.

```pleno
let person: (String, Int, Bool) = ("Alice", 30, true);

fn sumAndProduct(a: Int, b: Int): (Int, Int) {
    return (a + b, a * b);
}
print(sumAndProduct(3, 5));   // (8, 15)

let points: (Int, Int)[] = [(0, 0), (3, 4)];
print(points.length);   // 2

try {
    let wrong: (String, Int) = ("Bob", "thirty");
} catch (e) {
    print(e);           // TypeError: expected (String, Int), found Tuple
}
```

A function's return type annotation, `(Int, Int)` above, is checked when the function returns, like a parameter annotation.

## Destructuring

`let` can take a tuple or an array apart into names. A pattern is a name, `_` to skip a value, a tuple pattern `(a, b)` or an array pattern `[a, b]`, and patterns nest.

```pleno
let person = ("Alice", 30, true);
let (name, age, active) = person;
print(name, age, active);   // Alice 30 true

// `_` skips an element; patterns nest
let (_, (x, y)) = ("origin", (0, 0));
print(x + y);               // 0

// `let mut` makes every name mutable
let mut (low, high) = (10, 1);
if (low > high) {
    let (a, b) = (high, low);
    low = a;
    high = b;
}
print(low, high);           // 1 10

// Arrays, with an optional rest element at the end
let [first, second] = [1, 2];
let [head, ...tail] = [1, 2, 3, 4];
print(head, tail);          // 1 [2, 3, 4]
let [top, ...] = [9, 8, 7]; // a bare `...` ignores the rest
print(top);                 // 9

// Several results from a function
fn minMax(items: Int[]): (Int, Int) {
    let mut lowest = items[0];
    let mut highest = items[0];
    for (item in items) {
        if (item < lowest) { lowest = item; }
        if (item > highest) { highest = item; }
    }
    return (lowest, highest);
}
let (lowest, highest) = minMax([3, 9, 1, 4]);
print(lowest, highest);     // 1 9

// An annotation after the pattern checks the whole value
let (id, label): (Int, String) = (7, "seven");
print(label);               // seven
```

The pattern must fit the value, or the `let` fails with a runtime error:

- A tuple pattern needs a tuple (a named tuple works too, by position) with exactly as many elements: `let (a, b, c) = (1, 2);` fails with `tuple has 2 elements, pattern expects 3`.
- An array pattern needs an array with exactly as many elements. With a rest element, `[a, ...rest]`, the array needs at least as many elements as the names before the rest, and `rest` is a new array of the remaining ones (possibly empty).
- Any other value fails with `cannot destructure Int as a tuple` (or `as an array`).

```pleno
try {
    let (a, b, c) = (1, 2);
} catch (e) {
    print(e.message);   // tuple has 2 elements, pattern expects 3
}

try {
    let [first, second] = [1];
} catch (e) {
    print(e.message);   // array has 1 element, pattern expects 2
}

try {
    let (a, b) = 5;
} catch (e) {
    print(e.message);   // cannot destructure Int as a tuple
}
```

A tuple pattern with one element needs a trailing comma, `let (a,) = single;`, and a rest element is only allowed in array patterns. A destructuring `let` always has a value, and cannot be `lazy`, `deep` or exported. `for` loops do not take patterns: `for (key, value in map)` iterates a map's entries, and `for (pair in pairs) { let (a, b) = pair; ... }` destructures each element.

## Method reference

The methods of arrays, maps and sets not shown above:

```pleno
let arr = [1, 2, 3, 4, 5];
print(arr.pop());                    // 5: removes and returns the last element
print(arr.shift());                  // 1: removes and returns the first element
print(arr.remove(0));                // 2: removes and returns the element at an index
print(arr.slice(0, 1));              // [3]: elements from start to end (exclusive)
print(arr.join(", "));               // 3, 4
print(arr.find(|x| => x > 3));       // 4: the first match, or null
print(arr.with(0, 99));              // [99, 4]: a copy with one element replaced
print(arr.indexOf(42));              // -1 when missing
arr.clear();                         // removes every element

let map = {"a": 1, "b": 2};
print(map.remove("a"));              // true (false if the key was missing)
print(map.map(|k, v| => v * 2));     // {"b": 4}: a new map with new values
print(map.filter(|k, v| => v > 1));  // {"b": 2}
map.forEach(|k, v| => print(k));     // b
print(map.with("d", 4));             // {"b": 2, "d": 4}: a copy with an entry added
map.clear();

let set = Set([1, 2, 3]);
print(set.remove(2));                // true
print(set.toArray());                // [1, 3]
print(set.map(|x| => x * 2));        // [2, 6]: an array
print(set.filter(|x| => x > 1));     // {3}: a set
print(set.with(5));                  // {1, 3, 5}: a copy with an element added
set.clear();
```

`entries()` and `iterator()` on arrays and maps, and `iterator()` on sets, return iterator objects used by `for` loops.

## Ranges

```pleno
let range1 = 0..10;      // 0 to 9 (exclusive end)
let range2 = 0..=10;     // 0 to 10 (inclusive end)
let range3 = 5..;        // 5 and up
let range4 = ..10;       // everything below 10

// Range methods
print(range1.contains(5));    // true
print(range1.contains(10));   // false
print(range2.contains(10));   // true
print(range1.toArray());      // [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]

// Iteration
for (i in 1..=5) {
    print(i);  // 1, 2, 3, 4, 5
}

// Unbounded ranges iterate until you break
for (i in 5..) {
    if (i > 10) { break; }
    print(i);
}
```
