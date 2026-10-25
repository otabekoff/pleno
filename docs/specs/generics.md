# Generics

### 10.1 Generic Functions

```pleno
fn identity<T>(value: T): T {
    return value;
}

let num = identity<Int>(42);
let str = identity<String>("hello");
let inferred = identity(3.14);  // Type inferred as Float
```

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

### 10.3 Generic Constraints

```pleno
// Constraint: T must implement Comparable
fn findMax<T: Comparable>(items: T[]): T? {
    if (items.isEmpty()) return None;

    let mut max = items[0];
    for (item in items) {
        if (item > max) {
            max = item;
        }
    }
    return Some(max);
}

// Multiple constraints
fn process<T: Serializable & Cloneable>(item: T): T {
    let copy = item.clone();
    serialize(copy);
    return copy;
}
```

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

Pleno V2 supports variance annotations on generic parameters to enable safer and more flexible type relationships.

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

If no annotation is provided, the type is **invariant**. `Box<Dog>` cannot be assigned to `Box<Animal>` because a `Box` allows both reading and writing.
