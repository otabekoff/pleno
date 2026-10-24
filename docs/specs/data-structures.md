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
```

## Maps (Dictionaries)

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

::: warning Planned
Tuple type annotations such as `(String, Int, Bool)` are not implemented yet; the parser reports `Invalid type grouping`. Destructuring with `let (name, age, active) = person;` is not implemented either. Tuple values, `.0`/`.1` access and named tuples work.
:::

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
```

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
