## 2. Control Flow

### 2.1 Conditionals

#### If-Else
```pleno
if (condition) {
    // code
} else if (otherCondition) {
    // code
} else {
    // code
}

// If as expression
let status = if (score >= 60) "Pass" else "Fail";
```

#### Pattern Matching (Match Expression)
```pleno
match (value) {
    case 0 => print("zero"),
    case 1, 2, 3 => print("small"),
    case x if x > 100 => print("large: " + x),
    case String s => print("string: " + s),
    case _ => print("other")  // Default case
}

// Match as expression
let category = match (age) {
    case x if x < 13 => "child",
    case x if x < 20 => "teen",
    case x if x < 65 => "adult",
    case _ => "senior"
};
```

### 2.2 Loops

#### For Loop
```pleno
// Traditional for
for (let i = 0; i < 10; i++) {
    print(i);
}

// For-in (iteration)
for (item in collection) {
    print(item);
}

// For-in with index
for (index, item in collection) {
    print(index + ": " + item);
}

// Range-based
for (i in 0..10) {      // Exclusive end
    print(i);
}

for (i in 0..=10) {     // Inclusive end
    print(i);
}
```

#### While Loop
```pleno
while (condition) {
    // code
}

// Do-while
do {
    // code
} while (condition);
```

#### Loop Control
```pleno
break;        // Exit loop
continue;     // Skip to next iteration
break label;  // Break from labeled outer loop

outer: for (i in 0..10) {
    for (j in 0..10) {
        if (i * j > 50) break outer;
    }
}
```