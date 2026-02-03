# std:time

Date, time, and duration utilities.

## Import

```pleno
import { DateTime, Duration } from "std:time";
```

## DateTime

Represents a point in time.

### `now(): DateTime`

Returns the current date and time.

```pleno
let now = DateTime.now();
print(now.format("%Y-%m-%d %H:%M:%S"));
```

### `now_ns(): Int`

Returns current time as nanoseconds since Unix epoch.

```pleno
let nanos = DateTime.now_ns();
```

### `sleep(ms: Int): Void`

Pauses execution for the specified milliseconds.

```pleno
print("Starting...");
DateTime.sleep(1000);  // Wait 1 second
print("Done!");
```

### `parse(dateStr: String, fmt: String): DateTime`

Parses a date string using the specified format.

```pleno
let date = DateTime.parse("2026-02-03", "%Y-%m-%d");
```

### `format(fmt: String): String`

Formats the DateTime as a string.

| Specifier | Description    |
| --------- | -------------- |
| `%Y`      | 4-digit year   |
| `%m`      | Month (01-12)  |
| `%d`      | Day (01-31)    |
| `%H`      | Hour (00-23)   |
| `%M`      | Minute (00-59) |
| `%S`      | Second (00-59) |

```pleno
let dt = DateTime.now();
print(dt.format("%Y-%m-%d"));  // "2026-02-03"
```

### `add(duration: Duration): DateTime`

Returns a new DateTime with the duration added.

```pleno
let tomorrow = DateTime.now().add(Duration.days(1));
```

### `subtract(duration: Duration): DateTime`

Returns a new DateTime with the duration subtracted.

```pleno
let yesterday = DateTime.now().subtract(Duration.days(1));
```

---

## Duration

Represents a span of time.

### Static Constructors

```pleno
Duration.seconds(30);   // 30 seconds
Duration.minutes(5);    // 5 minutes
Duration.hours(2);      // 2 hours
Duration.days(7);       // 7 days
```

### `getSeconds(): Int`

Returns the duration in seconds.

```pleno
let d = Duration.hours(2);
print(d.getSeconds());  // 7200
```

---
