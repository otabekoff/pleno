## 11. Decorators/Attributes

```pleno
// Decorator definition
@decorator
fn logged(target: Function): Function {
    return fn(...args) => {
        print("Calling function with args: " + args);
        let result = target(...args);
        print("Result: " + result);
        return result;
    };
}

// Using decorator
@logged
fn calculate(x: Int, y: Int): Int {
    return x + y;
}

// Multiple decorators
@authenticated
@cached
@logged
fn getUserData(userId: Int): User {
    // implementation
}

// Class decorators
@serializable
@observable
class User {
    public name: String;
    public age: Int;
}
```