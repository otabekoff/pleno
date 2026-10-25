# std:time

> **Stability:** stable in 1.0.

Date, time, and duration utilities. All dates are in UTC.

## Import

```pleno
import { DateTime, Duration } from "std:time";
```

The module also exports plain functions that work on Unix timestamps: `now`, `now_ns`, `format`, `parse`, `parts`, `sleep`, and `setTimeout`. `import { Time } from "std:time"` gives all of them as one namespace.

## DateTime

Represents a point in time. A DateTime has the fields `year`, `month`, `day`, `hour`, `minute`, `second`, and `millisecond`, and the methods below.

### `DateTime.now(): DateTime`

Returns the current date and time.

```pleno
import { DateTime } from "std:time";

let now = DateTime.now();
print(now.format("%Y-%m-%d %H:%M:%S"));
```

### `DateTime.parse(text: String, fmt: String): DateTime`

Parses a date string using the given format. A format without a time of day gives midnight.

```pleno
import { DateTime } from "std:time";

let date = DateTime.parse("2026-02-03", "%Y-%m-%d");
print(date.year);     // 2026
print(date.toISO());  // 2026-02-03T00:00:00.000Z
```

### `DateTime.now_ns(): Int`

Returns the current time as nanoseconds since the Unix epoch.

```pleno
import { DateTime } from "std:time";

let nanos = DateTime.now_ns();
```

### `DateTime.sleep(ms: Int): Promise`

Returns a promise that resolves after the given number of milliseconds. Use `await` to pause.

```pleno
import { DateTime } from "std:time";

print("Starting...");
await DateTime.sleep(100);
print("Done!");
```

### `format(fmt: String): String`

Formats the DateTime as a string. The format uses `strftime` specifiers.

| Specifier | Description    |
| --------- | -------------- |
| `%Y`      | 4-digit year   |
| `%m`      | Month (01-12)  |
| `%d`      | Day (01-31)    |
| `%H`      | Hour (00-23)   |
| `%M`      | Minute (00-59) |
| `%S`      | Second (00-59) |

```pleno
import { DateTime } from "std:time";

let dt = DateTime.parse("2026-02-03 14:30:00", "%Y-%m-%d %H:%M:%S");
print(dt.format("%d.%m.%Y %H:%M"));  // 03.02.2026 14:30
```

### `toISO(): String`

Returns the date as an ISO 8601 string in UTC, with milliseconds.

```pleno
import { DateTime } from "std:time";

let dt = DateTime.parse("2026-02-03 14:30:00", "%Y-%m-%d %H:%M:%S");
print(dt.toISO());  // 2026-02-03T14:30:00.000Z
```

### `getTimestamp(): Int`

Returns the seconds since the Unix epoch.

```pleno
import { DateTime } from "std:time";

let dt = DateTime.parse("2026-02-03", "%Y-%m-%d");
print(dt.getTimestamp());  // 1770076800
```

### `add(duration: Duration): DateTime`

Returns a new DateTime with the duration added. A plain number counts as seconds.

```pleno
import { DateTime, Duration } from "std:time";

let tomorrow = DateTime.now().add(Duration.days(1));
```

### `subtract(duration: Duration): DateTime`

Returns a new DateTime with the duration subtracted.

```pleno
import { DateTime, Duration } from "std:time";

let yesterday = DateTime.now().subtract(Duration.days(1));
```

### `isBefore(other: DateTime): Bool` and `isAfter(other: DateTime): Bool`

Compare two DateTimes.

```pleno
import { DateTime, Duration } from "std:time";

let now = DateTime.now();
let later = now.add(Duration.hours(1));
print(now.isBefore(later));  // true
print(now.isAfter(later));   // false
```

---

## Duration

Represents a span of time.

### Static Constructors

```pleno
import { Duration } from "std:time";

Duration.milliseconds(500);
Duration.seconds(30);
Duration.minutes(5);
Duration.hours(2);
Duration.days(7);
```

### `getSeconds(): Int`

Returns the duration in whole seconds.

```pleno
import { Duration } from "std:time";

let d = Duration.hours(2);
print(d.getSeconds());  // 7200
```

---

## Timestamp Functions

These work on Unix timestamps in seconds, as `Float`.

### `now(): Float`

Returns the current time in seconds since the Unix epoch, with a fractional part.

```pleno
import { now } from "std:time";

let start = now();
```

### `format(timestamp: Float, fmt: String): String`

Formats a timestamp in UTC.

```pleno
import { format } from "std:time";

print(format(0, "%Y-%m-%d"));  // 1970-01-01
```

### `parse(text: String, fmt: String): Float`

Parses a date and time string into a timestamp. The format must include a time of day.

```pleno
import { parse } from "std:time";

print(parse("2026-02-03 00:00:00", "%Y-%m-%d %H:%M:%S"));  // 1770076800
```

### `parts(timestamp: Float): Map`

Splits a timestamp into its UTC parts.

```pleno
import { parts } from "std:time";

let p = parts(1770076800);
print(p.year, p.month, p.day);  // 2026 2 3
```

### `sleep(ms: Int): Promise`

Same as `DateTime.sleep`.

```pleno
import { sleep } from "std:time";

await sleep(50);
```

### `setTimeout(callback: Function, ms: Int, ...args): Int`

Calls `callback` with `args` after `ms` milliseconds. Returns the timer id.

```pleno
import { setTimeout } from "std:time";

setTimeout(fn(name) => print(`hello, ${name}`), 10, "Ann");
print("scheduled");
// scheduled
// hello, Ann
```
