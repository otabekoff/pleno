# std:math

The Math module provides mathematical constants and functions.

## Import

Import the `Math` namespace, or import single names:

```pleno
import { Math } from "std:math";
import { sqrt, PI } from "std:math";

print(Math.sqrt(9));  // 3
print(sqrt(16));      // 4
print(PI);            // 3.141592653589793
```

Functions accept both `Int` and `Float` arguments.

## Constants

### `PI`

The mathematical constant π (approximately 3.14159).

```pleno
import { Math } from "std:math";

let radius = 2;
let area = Math.PI * radius * radius;
print(area);  // 12.566370614359172
```

### `E`

Euler's number e (approximately 2.71828).

```pleno
import { Math } from "std:math";

print(Math.E);       // 2.718281828459045
print(Math.exp(1));  // 2.718281828459045
```

### Other constants

`LN2`, `LN10`, `LOG2E`, `LOG10E`, `SQRT2` and `SQRT1_2` are also available.

```pleno
import { Math } from "std:math";

print(Math.SQRT2);  // 1.4142135623730951
print(Math.LN2);    // 0.6931471805599453
```

## Functions

### `abs(x: Float): Float`

Returns the absolute value of `x`. An `Int` argument gives an `Int` result.

```pleno
import { Math } from "std:math";

print(Math.abs(-5));    // 5
print(Math.abs(-3.14)); // 3.14
```

### `floor(x: Float): Int`

Rounds `x` down to the nearest integer.

```pleno
import { Math } from "std:math";

print(Math.floor(3.7));   // 3
print(Math.floor(-2.3));  // -3
```

### `ceil(x: Float): Int`

Rounds `x` up to the nearest integer.

```pleno
import { Math } from "std:math";

print(Math.ceil(3.2));   // 4
print(Math.ceil(-2.7));  // -2
```

### `round(x: Float): Int`

Rounds `x` to the nearest integer. Halfway values round away from zero.

```pleno
import { Math } from "std:math";

print(Math.round(3.5));   // 4
print(Math.round(3.4));   // 3
print(Math.round(-2.5));  // -3
```

### `trunc(x: Float): Int`

Drops the fractional part of `x`.

```pleno
import { Math } from "std:math";

print(Math.trunc(3.7));   // 3
print(Math.trunc(-3.7));  // -3
```

### `sign(x: Float): Int`

Returns `1` for positive numbers, `-1` for negative numbers and `0` for zero.

```pleno
import { Math } from "std:math";

print(Math.sign(-4));   // -1
print(Math.sign(0));    // 0
print(Math.sign(2.5));  // 1
```

### `sqrt(x: Float): Float`

Returns the square root of `x`.

```pleno
import { Math } from "std:math";

print(Math.sqrt(16));  // 4
print(Math.sqrt(2));   // 1.4142135623730951
```

### `pow(base: Float, exp: Float): Float`

Returns `base` raised to the power of `exp`.

```pleno
import { Math } from "std:math";

print(Math.pow(2, 10));   // 1024
print(Math.pow(3, 0.5));  // 1.7320508075688772
```

### `exp(x: Float): Float`

Returns e raised to the power of `x`.

```pleno
import { Math } from "std:math";

print(Math.exp(0));  // 1
```

### `sin(x: Float): Float`

Returns the sine of `x` (in radians).

```pleno
import { Math } from "std:math";

print(Math.sin(Math.PI / 2));  // 1
```

### `cos(x: Float): Float`

Returns the cosine of `x` (in radians).

```pleno
import { Math } from "std:math";

print(Math.cos(0));        // 1
print(Math.cos(Math.PI));  // -1
```

### `tan(x: Float): Float`

Returns the tangent of `x` (in radians).

```pleno
import { Math } from "std:math";

print(Math.tan(Math.PI / 4));  // 0.9999999999999999
```

### `atan2(y: Float, x: Float): Float`

Returns the angle in radians between the positive x axis and the point `(x, y)`.

```pleno
import { Math } from "std:math";

print(Math.atan2(1, 1));  // 0.7853981633974483
```

### `log(x: Float): Float`

Returns the natural logarithm (base e) of `x`.

```pleno
import { Math } from "std:math";

print(Math.log(Math.E));  // 1
print(Math.log(10));      // 2.302585092994046
```

### `log10(x: Float): Float`

Returns the base-10 logarithm of `x`.

```pleno
import { Math } from "std:math";

print(Math.log10(100));  // 2
```

### `max(a: Float, b: Float): Float`

Returns the larger of `a` and `b`. The result is always a `Float`.

```pleno
import { Math } from "std:math";

print(Math.max(5, 10));  // 10
```

### `min(a: Float, b: Float): Float`

Returns the smaller of `a` and `b`. The result is always a `Float`.

```pleno
import { Math } from "std:math";

print(Math.min(5, 10));  // 5
```

### `random(): Float`

Returns a random floating-point number between 0 (inclusive) and 1 (exclusive).

```pleno
import { Math } from "std:math";

let r = Math.random();  // e.g. 0.4024525217869972
```

### `randomInt(min: Int, max: Int): Int`

Returns a random integer between `min` and `max`, both inclusive. `min` must be less than `max`.

```pleno
import { Math } from "std:math";

let dice = Math.randomInt(1, 6);  // 1 to 6
print(dice);
```

---
