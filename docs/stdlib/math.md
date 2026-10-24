# std:math

The Math module provides mathematical constants and functions.

## Import

```pleno
import { Math } from "std:math";
```

## Constants

### `PI`

The mathematical constant π (approximately 3.14159).

```pleno
let area = Math.PI * radius * radius;
```

### `E`

Euler's number e (approximately 2.71828).

```pleno
let growth = Math.pow(Math.E, x);  // same as Math.exp(x)
```

## Functions

### `abs(x: Float): Float`

Returns the absolute value of `x`.

```pleno
Math.abs(-5);    // 5
Math.abs(3.14);  // 3.14
```

### `floor(x: Float): Int`

Rounds `x` down to the nearest integer.

```pleno
Math.floor(3.7);   // 3
Math.floor(-2.3);  // -3
```

### `ceil(x: Float): Int`

Rounds `x` up to the nearest integer.

```pleno
Math.ceil(3.2);   // 4
Math.ceil(-2.7);  // -2
```

### `round(x: Float): Int`

Rounds `x` to the nearest integer.

```pleno
Math.round(3.5);  // 4
Math.round(3.4);  // 3
```

### `sqrt(x: Float): Float`

Returns the square root of `x`.

```pleno
Math.sqrt(16);  // 4.0
Math.sqrt(2);   // 1.414...
```

### `pow(base: Float, exp: Float): Float`

Returns `base` raised to the power of `exp`.

```pleno
Math.pow(2, 10);  // 1024.0
Math.pow(3, 0.5); // 1.732...
```

### `sin(x: Float): Float`

Returns the sine of `x` (in radians).

```pleno
Math.sin(Math.PI / 2);  // 1.0
```

### `cos(x: Float): Float`

Returns the cosine of `x` (in radians).

```pleno
Math.cos(0);       // 1.0
Math.cos(Math.PI); // -1.0
```

### `tan(x: Float): Float`

Returns the tangent of `x` (in radians).

```pleno
Math.tan(Math.PI / 4);  // 1.0
```

### `log(x: Float): Float`

Returns the natural logarithm (base e) of `x`.

```pleno
Math.log(Math.E);  // 1.0
Math.log(10);      // 2.302...
```

### `log10(x: Float): Float`

Returns the base-10 logarithm of `x`.

```pleno
Math.log10(100);  // 2.0
```

### `max(a: Float, b: Float): Float`

Returns the larger of `a` and `b`.

```pleno
Math.max(5, 10);  // 10
```

### `min(a: Float, b: Float): Float`

Returns the smaller of `a` and `b`.

```pleno
Math.min(5, 10);  // 5
```

### `random(): Float`

Returns a random floating-point number between 0 (inclusive) and 1 (exclusive).

```pleno
let r = Math.random();  // e.g., 0.7234...
```

### `randomInt(min: Int, max: Int): Int`

Returns a random integer between `min` (inclusive) and `max` (exclusive).

```pleno
let dice = Math.randomInt(1, 7);  // 1-6
```

---
