# Generics

### 10.1 Generic Functions

```pleno
fn identity<T>(value: T): T {
    return value;
}

let num = identity<Int>(42);
let str = identity<String>("hello");
let inferred = identity(3.14);  // No type argument: T is not checked
```

With explicit type arguments, parameters annotated with the type parameter are checked: `identity<Int>("s")` throws `TypeError: parameter 'value' expected T, got String`. Without them, a type parameter accepts any value.

### 10.2 Generic Classes

```pleno
class Box<T> {
    private value: T;

    constructor(value: T) {
        this.value = value;
    }

    public fn get(): T {
        return this.value;
    }

    public fn set(value: T): Void {
        this.value = value;
    }
}

let intBox = Box<Int>(42);
let strBox = Box<String>("hello");
```

An instance remembers the type arguments it was created with, or the types of the constructor arguments when none are given, and an annotation such as `Box<Int>` checks them: `let b: Box<Int> = Box("s");` throws `TypeError: expected Box<Int>, found Box`.

### 10.3 Generic Constraints

`T: Name` requires the type argument to be, extend or implement `Name`, a class or interface you declare, and `T: A & B` requires both. The constraint is checked with explicit type arguments (`greet<Rock>(...)`) and with inferred ones: a parameter annotated `T` must hold a value that satisfies it (`parameter 'x' expected T: Named, got Rock`). A parameter annotated `T[]` does not check its elements. A constraint naming something undefined rejects every type argument.

`Comparable` is built in: `T: Comparable` accepts `Int`, `Float`, `String` and `Char`, and classes that define `operator <` or `operator >` or implement an interface named `Comparable`, the types `<` and `>` work on.

```pleno
interface Shape {
    fn area(): Float;
}

class Square implements Shape {
    private side: Float;
    constructor(side: Float) { this.side = side; }
    fn area(): Float { return this.side * this.side; }
}

fn totalArea<T: Shape>(shapes: T[]): Float {
    let mut total = 0.0;
    for (shape in shapes) {
        total += shape.area();
    }
    return total;
}

print(totalArea<Square>([Square(2.0), Square(3.0)]));   // 13.0

try {
    totalArea<Int>([1, 2]);
} catch (e) {
    print(e.message);
    // Generic constraint failed: Type 'Int' does not satisfy constraint 'Shape' for parameter 'T' in function 'totalArea'
}
```

Without explicit type arguments, only a parameter annotated with the type parameter itself (`x: T`) is checked against the constraint. `totalArea([1, 2])` has a `T[]` parameter, so nothing is checked when it is called, and it fails later, inside the function (`cannot read property 'area' of Int`).

### 10.4 Generic Interfaces

```pleno
interface Container<T> {
    fn add(item: T): Void;
    fn get(index: Int): T?;
    fn size(): Int;
}

class ArrayList<T> implements Container<T> {
    private items: T[] = [];

    fn add(item: T): Void {
        this.items.push(item);
    }

    fn get(index: Int): T? {
        if (index >= 0 && index < this.items.length) {
            return Some(this.items[index]);
        }
        return None;
    }

    fn size(): Int {
        return this.items.length;
    }
}
```

### 10.5 Variance Annotations

Variance annotations on the type parameters of an interface say when an instance of one generic type matches an annotation with different type arguments. They are checked at runtime, when a value is checked against an annotation such as `Producer<Animal>`.

#### Covariance (`out`)

Use `out` when a type parameter only appears in **output** positions (return types). This allows a more specific type to be used where a more general one is expected.

```pleno
class Animal {}
class Dog extends Animal {}

interface Producer<out T> {
    fn produce(): T;
}

class AnimalProducer implements Producer<Animal> {
    fn produce(): Animal { /* ... */ }
}
class DogProducer implements Producer<Dog> {
    fn produce(): Dog { /* ... */ }
}

let p: Producer<Animal> = DogProducer(); // Valid due to 'out'
```

#### Contravariance (`in`)

Use `in` when a type parameter only appears in **input** positions (argument types). This allows a more general type to be used where a more specific one is expected.

```pleno
class Animal {}
class Dog extends Animal {}

interface Consumer<in T> {
    fn consume(item: T): Void;
}

class AnimalConsumer implements Consumer<Animal> {
    fn consume(item: Animal): Void { /* ... */ }
}
class DogConsumer implements Consumer<Dog> {
    fn consume(item: Dog): Void { /* ... */ }
}

let c: Consumer<Dog> = AnimalConsumer(); // Valid due to 'in'
```

#### Invariance (Default)

If no annotation is provided, the type is **invariant**: `let b: Box<Animal> = Box<Dog>(Dog());` throws `TypeError: expected Box<Animal>, found Box`, because a `Box` allows both reading and writing.
