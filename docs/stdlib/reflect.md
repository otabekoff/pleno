# std:reflect

Runtime reflection utilities for type inspection and dynamic access.

## Import

```pleno
import { Reflect } from "std:reflect";
```

## Type Inspection

### `getTypeName(value: Any): String`

Returns the type name of a value.

```pleno
Reflect.getTypeName(42);           // "Int"
Reflect.getTypeName("hello");      // "String"
Reflect.getTypeName(someObject);   // "Person" (class name)
```

### `getFields(value: Any): String[]`

Returns the names of all fields on an object.

```pleno
let person = new Person("Alice", 30);
let fields = Reflect.getFields(person);  // ["name", "age"]
```

### `getMethods(value: Any): String[]`

Returns the names of all methods on an object or class.

```pleno
let methods = Reflect.getMethods(Person);
// ["constructor", "greet", "toString"]
```

---

## Dynamic Access

### `getField(obj: Object, name: String): Any`

Gets a field value by name.

```pleno
let name = Reflect.getField(person, "name");  // "Alice"
```

### `setField(obj: Object, name: String, value: Any): Void`

Sets a field value by name.

```pleno
Reflect.setField(person, "age", 31);
```

### `callMethod(obj: Object, name: String, args: Array): Any`

Calls a method by name with the given arguments.

```pleno
let result = Reflect.callMethod(calculator, "add", [1, 2]);
```

---

## Type Checking

### `isInstanceOf(value: Any, type: Class): Bool`

Checks if a value is an instance of the given type.

```pleno
Reflect.isInstanceOf(person, Person);  // true
Reflect.isInstanceOf(person, Animal);  // false
```

---

## Annotations

### `getAnnotations(target: Any): Array<Map>`

Returns all annotations on a class, enum, or object.

```pleno
@Entity
@Table("users")
class User { ... }

let annotations = Reflect.getAnnotations(User);
// [{ name: "Entity", args: [] }, { name: "Table", args: ["users"] }]
```

---
