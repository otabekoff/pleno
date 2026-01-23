## 7. Error Handling

### 7.1 Try-Catch

```pleno
try {
    // Code that might throw
    let result = riskyOperation();
} catch (e: NetworkError) {
    print("Network error: " + e.message);
} catch (e: ValidationError) {
    print("Validation error: " + e.message);
} catch (e: Error) {
    print("General error: " + e.message);
} finally {
    // Always executes
    cleanup();
}
```

### 7.2 Custom Exceptions

```pleno
class ValidationError extends Error {
    private field: String;
    
    constructor(message: String, field: String) {
        super(message);
        this.field = field;
    }
    
    public fn getField(): String {
        return this.field;
    }
}

fn validateAge(age: Int) {
    if (age < 0 || age > 150) {
        throw ValidationError("Invalid age", "age");
    }
}
```

### 7.3 Result Type (Functional Error Handling)

```pleno
// Result type for functions that might fail
fn divide(a: Float, b: Float): Result<Float, String> {
    if (b == 0.0) {
        return Err("Division by zero");
    }
    return Ok(a / b);
}

// Using Result
let result = divide(10.0, 2.0);
match (result) {
    case Ok(value) => print("Result: " + value),
    case Err(error) => print("Error: " + error)
}

// Result methods
let value = result.unwrap();           // Panics if Err
let value = result.unwrapOr(0.0);     // Default value if Err
let value = result.expect("Failed");   // Panics with message if Err
```

### 7.4 Option Type

```pleno
fn findUser(id: Int): Option<User> {
    // Returns Some(user) or None
    if (userExists(id)) {
        return Some(getUser(id));
    }
    return None;
}

// Using Option
let user = findUser(123);
match (user) {
    case Some(u) => print("Found: " + u.name),
    case None => print("Not found")
}

// Option methods
user.unwrap();              // Panics if None
user.unwrapOr(defaultUser); // Default if None
user.isSome();              // Check if Some
user.isNone();              // Check if None
```
