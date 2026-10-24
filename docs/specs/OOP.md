# Object-Oriented Programming

## Class Definition

```pleno
import { Math } from "std:math";

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
        this.id = this.generateId();
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
print(person.greet());  // Hello, I'm Alice
print(person.active);   // true: public field

try {
    print(person.name);
} catch (e) {
    print(e.message);   // field access denied
}
```

## Properties (Getters/Setters)

```pleno
import { Math } from "std:math";

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
print(circle.area);      // 78.53981633974483 (computed)
circle.diameter = 20.0;  // Sets radius to 10
print(circle.diameter);  // 20
```

## Inheritance

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
print(dog.speak());  // Woof! I'm Buddy
```

## Abstract Classes

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

let rect = Rectangle("blue", 10.0, 5.0);
print(rect.area());      // 50
print(rect.describe());  // A blue shape

try {
    Shape("red");
} catch (e) {
    print(e.message);  // cannot instantiate abstract class 'Shape'
}
```

## Interfaces

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

let canvas = Canvas("red", 1.0);
canvas.draw();               // Drawing canvas
print(canvas.getColor());    // red
```

A class that does not define every method of its interfaces fails when the class is declared: `Class 'Canvas' missing method 'draw' required by interface 'Drawable'`.

## Static Members

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
print(MathUtils.PI);             // 3.14159
print(MathUtils.square(5.0));    // 25

MathUtils();
MathUtils();
print(MathUtils.getInstanceCount());  // 2
```

## Access Modifiers

- `public` - Accessible everywhere (the default for fields and methods)
- `private` - Accessible only within the class
- `protected` - Accessible within the class and its subclasses
- `internal` - Accessible within the same module

Access modifiers are checked at runtime for methods and fields, including static fields. A denied access throws `field access denied`, `method access denied` or `static field access denied`.

```pleno
class Example {
    public publicField: Int;
    private privateField: Int;
    protected protectedField: Int;
    internal internalField: Int;
}
```
