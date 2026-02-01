# test_interface_properties

## Contents

- [ILogger](#ilogger)
- [ConsoleLogger](#consolelogger)

## ILogger

```pleno
interface ILogger
```

### Properties

#### `get` `set` `level: Int`

### Methods

#### `abstract` `fn log(message: String)`

---

## ConsoleLogger

```pleno
class ConsoleLogger implements ILogger
```

### Fields

#### `_level: Int`

### Properties

#### `get` `set` `level: Int`

### Methods

#### `fn log(message: String)`

---

