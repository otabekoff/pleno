# std:reflect

Runtime reflection utilities for type inspection and dynamic access.

## Import

```pleno
import { Reflect } from "std:reflect";
```

The functions can also be imported one by one, e.g. `import { getTypeName } from "std:reflect"`.

The examples below use this class:

```pleno
class Person {
    public name: String;
    public age: Int;

    fn constructor(name: String, age: Int) {
        this.name = name;
        this.age = age;
    }

    fn greet(greeting: String): String {
        return `${greeting}, ${this.name}`;
    }
}
```

## Type Inspection

### `getTypeName(value: Any): String`

Returns the type name of a value. For an object, it returns its class name.

```pleno
import { Reflect } from "std:reflect";

class Person {}

print(Reflect.getTypeName(42));        // Int
print(Reflect.getTypeName("hello"));   // String
print(Reflect.getTypeName([1, 2]));    // Array
print(Reflect.getTypeName(Person()));  // Person
```

### `getFields(value: Object): String[]`

Returns the field names of an object, sorted. Returns an empty array for values that are not objects.

```pleno
import { Reflect } from "std:reflect";

class Person {
    public name: String;
    public age: Int;
    fn constructor(name: String, age: Int) {
        this.name = name;
        this.age = age;
    }
}

let person = Person("Alice", 30);
print(Reflect.getFields(person));  // [age, name]
```

### `getMethods(value: Class | Object): String[]`

Returns the method names of a class or of an object's class, sorted. Static methods are included.

```pleno
import { Reflect } from "std:reflect";

class Person {
    fn constructor() {}
    fn greet(): String { return "hi"; }
    static fn create(): Person { return Person(); }
}

print(Reflect.getMethods(Person));  // [constructor, create, greet]
```

---

## Dynamic Access

### `getField(obj: Object, name: String): Any`

Gets a field value by name. Returns `null` if the field is not set.

```pleno
import { Reflect } from "std:reflect";

class Person {
    public name: String;
    fn constructor(name: String) { this.name = name; }
}

let person = Person("Alice");
print(Reflect.getField(person, "name"));  // Alice
```

### `setField(obj: Object, name: String, value: Any): Void`

Sets a field value by name.

```pleno
import { Reflect } from "std:reflect";

class Person {
    public age: Int;
    fn constructor(age: Int) { this.age = age; }
}

let person = Person(30);
Reflect.setField(person, "age", 31);
print(person.age);  // 31
```

### `callMethod(obj: Object, name: String, args: Array): Any`

Calls a method by name with the given arguments, and returns its result. Works on object instances only.

```pleno
import { Reflect } from "std:reflect";

class Calculator {
    fn add(a: Int, b: Int): Int { return a + b; }
}

let calculator = Calculator();
print(Reflect.callMethod(calculator, "add", [1, 2]));  // 3
```

---

## Type Checking

### `isInstanceOf(value: Any, type: Class): Bool`

Checks if a value is an instance of the given type.

```pleno
import { Reflect } from "std:reflect";

class Person {}
class Animal {}

let person = Person();
print(Reflect.isInstanceOf(person, Person));  // true
print(Reflect.isInstanceOf(person, Animal));  // false
```

---

## Annotations

### `getAnnotations(target: Class | Enum | Object): Array<Map>`

Returns the annotations on a class or enum. For an object, it returns the annotations on its class. Each entry is a map with `name` and `args`.

```pleno
import { Reflect } from "std:reflect";

@Entity
@Table("users")
class User {
    public id: Int = 0;
}

let annotations = Reflect.getAnnotations(User);
print(annotations.length);   // 2
print(annotations[0].name);  // Entity
print(annotations[1].args);  // [users]
```
