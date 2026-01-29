# time

## Contents

- [DateTime](#datetime)
- [Duration](#duration)

## DateTime

```pleno
class DateTime
```

### Fields

#### 🔒 `timestamp: Int`

### Methods

#### `fn constructor(timestamp: Int)`

#### `static` `fn now() -> DateTime`

#### `static` `fn now_ns() -> Int`

#### `static` `fn sleep(ms: Int)`

#### `static` `fn parse(dateStr: String, fmt: String) -> DateTime`

#### `fn format(fmt: String) -> String`

#### `fn add(duration: Duration) -> DateTime`

#### `fn subtract(duration: Duration) -> DateTime`

#### `fn getTimestamp() -> Int`

---

## Duration

```pleno
class Duration
```

### Fields

#### 🔒 `seconds: Int`

### Methods

#### `fn constructor(seconds: Int)`

#### `static` `fn seconds(s: Int) -> Duration`

#### `static` `fn minutes(m: Int) -> Duration`

#### `static` `fn hours(h: Int) -> Duration`

#### `static` `fn days(d: Int) -> Duration`

#### `fn getSeconds() -> Int`

---

