# Object-Oriented Programming

### 4.1 Class Definition

```pleno
class Person {
    // Private fields (truly encapsulated)
    private name: String;
    private age: Int;
    
    // Protected field (accessible in subclasses)
    protected id: String;
    
    // Public field
    public active: Bool;
    
    // Constructor
    constructor(name: String, age: Int) {
        this.name = name;
        this.age = age;
        this.id = generateId();
        this.active = true;
    }
    
    // Public method
    public fn greet(): String {
        return "Hello, I'm " + this.name;
    }
    
    // Private method
    private fn generateId(): String {
        return "ID_" + Math.random().toString();
    }
    
    // Getter
    public fn getName(): String {
        return this.name;
    }
    
    // Setter with validation
    public fn setAge(newAge: Int) {
        if (newAge >= 0 && newAge <= 150) {
            this.age = newAge;
        } else {
            throw Error("Invalid age");
        }
    }
}

// Usage
let person = Person("Alice", 30);
print(person.greet());
// print(person.name);  // ERROR: Cannot access private field
print(person.active);   // OK: Public field
```

### 4.2 Properties (Getters/Setters)

```pleno
class Circle {
    private radius: Float;
    
    constructor(radius: Float) {
        this.radius = radius;
    }
    
    // Property with getter
    property area: Float {
        get {
            return Math.PI * this.radius * this.radius;
        }
    }
    
    // Property with getter and setter
    property diameter: Float {
        get {
            return this.radius * 2;
        }
        set(value) {
            this.radius = value / 2;
        }
    }
}

let circle = Circle(5.0);
print(circle.area);      // 78.54 (computed)
circle.diameter = 20.0;  // Sets radius to 10
```

### 4.3 Inheritance

```pleno
class Animal {
    protected name: String;
    
    constructor(name: String) {
        this.name = name;
    }
    
    public fn speak(): String {
        return "Some sound";
    }
}

class Dog extends Animal {
    private breed: String;
    
    constructor(name: String, breed: String) {
        super(name);  // Call parent constructor
        this.breed = breed;
    }
    
    // Override parent method
    @override
    public fn speak(): String {
        return "Woof! I'm " + this.name;
    }
    
    public fn getBreed(): String {
        return this.breed;
    }
}

let dog = Dog("Buddy", "Golden Retriever");
print(dog.speak());  // "Woof! I'm Buddy"
```

### 4.4 Abstract Classes

```pleno
abstract class Shape {
    protected color: String;
    
    constructor(color: String) {
        this.color = color;
    }
    
    // Abstract method (must be implemented by subclasses)
    abstract fn area(): Float;
    
    // Concrete method
    public fn describe(): String {
        return "A " + this.color + " shape";
    }
}

class Rectangle extends Shape {
    private width: Float;
    private height: Float;
    
    constructor(color: String, width: Float, height: Float) {
        super(color);
        this.width = width;
        this.height = height;
    }
    
    @override
    fn area(): Float {
        return this.width * this.height;
    }
}

// let shape = Shape("red");  // ERROR: Cannot instantiate abstract class
let rect = Rectangle("blue", 10.0, 5.0);
print(rect.area());  // 50.0
```

### 4.5 Interfaces

```pleno
interface Drawable {
    fn draw(): Void;
    fn getColor(): String;
}

interface Resizable {
    fn resize(factor: Float): Void;
}

// Multiple interface implementation
class Canvas implements Drawable, Resizable {
    private color: String;
    private size: Float;
    
    constructor(color: String, size: Float) {
        this.color = color;
        this.size = size;
    }
    
    fn draw(): Void {
        print("Drawing canvas");
    }
    
    fn getColor(): String {
        return this.color;
    }
    
    fn resize(factor: Float): Void {
        this.size *= factor;
    }
}
```

### 4.6 Static Members

```pleno
class MathUtils {
    // Static constant
    public static const PI: Float = 3.14159;
    
    // Static field
    private static instanceCount: Int = 0;
    
    constructor() {
        MathUtils.instanceCount++;
    }
    
    // Static method
    public static fn square(x: Float): Float {
        return x * x;
    }
    
    public static fn getInstanceCount(): Int {
        return MathUtils.instanceCount;
    }
}

// Usage without instance
print(MathUtils.PI);
print(MathUtils.square(5.0));
MathUtils.version = "1.0";
print(MathUtils.version);
```

### 4.7 Access Modifiers

- `public` - Accessible everywhere (default for methods)
- `private` - Accessible only within the class
- `protected` - Accessible within class and subclasses
- `internal` - Accessible within the same module

Access modifiers are enforced for methods and class fields (including static fields), including `internal`.

```pleno
class Example {
    public publicField: Int;
    private privateField: Int;
    protected protectedField: Int;
    internal internalField: Int;
}
```
