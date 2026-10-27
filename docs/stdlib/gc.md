# std:gc

> **Stability:** experimental. Not covered by the [1.x compatibility promise](../specs/compatibility.md).

A view of the VM's memory manager. Pleno frees most values as soon as nothing refers to them (reference counting), and a collector frees groups of values that refer to each other, such as an array that contains itself. The collector runs on its own; this module lets a program run it now and see what it holds. See [Memory management](../specs/memory-performance.md).

::: warning Experimental
The numbers depend on how the VM stores values, which can change in any release. Use them to compare two moments of the same run, not as fixed sizes.
:::

## Import

```pleno
import { GC } from "std:gc";
import { collect, heapSize, isEnabled, report, stats } from "std:gc";
```

## Functions

### `collect(): Null`

Runs the collector now and frees every value that only other unreachable values refer to.

```pleno
import { GC } from "std:gc";

for (i in 0..1000) {
    let loop = [i];
    loop.push(loop);      // a cycle: only the collector frees it
}

let before = GC.heapSize();
GC.collect();
print(GC.heapSize() < before);   // true
```

### `heapSize(): Int`

The number of values the memory manager currently tracks (arrays, maps, objects, closures and other heap values; not Ints, Floats, Bools or `null`).

### `isEnabled(): Bool`

Whether the collector runs. It cannot be turned off, so this is always `true`.

### `stats(): Map`

A map of counters:

| Key | Meaning |
| --- | --- |
| `collections` | how many times the collector has run |
| `current_objects` | the same number as `heapSize()` |
| `current_bytes` | an estimate of the bytes the tracked values use, recomputed at each collection |
| `peak_bytes` | the highest estimate seen while allocating; it is not always comparable with `current_bytes` |
| `total_freed_bytes`, `total_freed_objects` | how much the collector has freed in all |
| `last_freed_bytes` | how much the last collection freed |

```pleno
import { GC } from "std:gc";

let before = GC.stats();
GC.collect();
let after = GC.stats();
print(after.collections - before.collections);   // 1
print(typeof(after.peak_bytes));               // Int
```

### `report(): Null`

Prints the same counters to standard output, one per line, for a person to read. The format of these lines can change in any release; read `stats()` in a program.

```pleno norun
import { GC } from "std:gc";

GC.report();
// --- Pleno GC Report ---
// Collections:       0
// Heap Size:         72 objects
// ...
```

For the instructions a program has run and its peak memory, see `vmStats()` in [std:reflect](reflect.md).
