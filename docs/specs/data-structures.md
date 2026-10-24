# Data Structures

### 5.1 Arrays

```pleno
// Array literal
let numbers: Int[] = [1, 2, 3, 4, 5];
let mixed: Any[] = [1, "two", 3.0, true];

// Array operations
numbers.push(6);           // Add to end
numbers.pop();             // Remove from end
numbers.unshift(0);        // Add to start
numbers.shift();           // Remove from start
numbers.insert(2, 99);     // Insert at index
numbers.remove(1);         // Remove at index

// Array methods
numbers.length;            // Get length
numbers.isEmpty();         // Check if empty
numbers.contains(3);       // Check if contains element
numbers.indexOf(4);        // Find index of element
numbers.reverse();         // Reverse in place
numbers.sort();            // Sort in place

// Functional methods
numbers.map(|x| => x * 2);
numbers.filter(|x| => x > 3);
numbers.reduce(|acc, x| => acc + x, 0);
numbers.forEach(|x| => print(x));

// Array slicing
let slice = numbers[1..4];    // Elements 1, 2, 3
let slice2 = numbers[2..];    // From index 2 to end
let slice3 = numbers[..3];    // From start to index 3
```

### 5.2 Maps (Dictionaries)

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
ages.put("David", 28);        // Add/update entry
ages.remove("Bob");           // Remove entry
ages.get("Alice");            // Get value (returns Int?)
ages.containsKey("Charlie");  // Check if key exists
ages.keys();                  // Get all keys
ages.values();                // Get all values
ages.entries();               // Get all key-value pairs
ages.size();                  // Get size
ages.isEmpty();               // Check if empty

// Safe access
let age = ages["Alice"] ?? 0;  // Default value if not found

// Iteration
for (key, value in ages) {
    print(key + ": " + value);
}
```

### 5.3 Sets

```pleno
let uniqueNumbers: Set<Int> = {1, 2, 3, 4, 5};

// Set operations
uniqueNumbers.add(6);
uniqueNumbers.remove(3);
uniqueNumbers.contains(4);
uniqueNumbers.size();
uniqueNumbers.isEmpty();

// Set operations
let set1 = {1, 2, 3};
let set2 = {3, 4, 5};

set1.union(set2);           // {1, 2, 3, 4, 5}
set1.intersection(set2);    // {3}
set1.difference(set2);      // {1, 2}
```

### 5.4 Tuples

::: warning Planned
Tuple type annotations such as `(String, Int, Bool)` and destructuring with `let (name, age, active) = person;` are not implemented yet. Tuples, `.0`/`.1` access and named tuples work.
:::

```pleno
// Tuple with different types
let person: (String, Int, Bool) = ("Alice", 30, true);

// Destructuring
let (name, age, active) = person;

// Access by index
print(person.0);  // "Alice"
print(person.1);  // 30

// Named tuples
let point: (x: Float, y: Float) = (x: 10.5, y: 20.3);
print(point.x);   // 10.5
```

### 5.5 Ranges

```pleno
let range1 = 0..10;      // 0 to 9 (exclusive end)
let range2 = 0..=10;     // 0 to 10 (inclusive end)
let range3 = 5..;        // 5 to infinity
let range4 = ..10;       // negative infinity to 9

// Range methods
range1.contains(5);      // true
range1.toArray();        // [0, 1, 2, ..., 9]

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