# std:csv

> **Stability:** stable in 1.0.

CSV parsing.

## Import

```pleno
import { CSV } from "std:csv";
```

`parse` can also be imported directly.

## Functions

### `parse(csv: String, options?: Map): Array<Array<String>>`

Parses a CSV string into an array of rows. Each row is an array of strings. Quoted fields may contain commas.

By default the first row is the header and is not included in the result, so input with only one row gives `[]`. Pass `{ header: false }` (or the named argument `header: false`) when the first line is data: every row is returned, the first one included. `header` is the only option; another key, or a `header` that is not a Bool, throws an error.

Rows with a different number of fields than the first row throw an error, for example `CSV error: record 1 (line: 2, byte: 4): found record with 1 fields, but the previous record has 2 fields`.

```pleno
import { CSV } from "std:csv";

let data = CSV.parse(`name,age,city
Alice,30,NYC
Bob,25,LA
"Smith, J",35,Chicago
`);

// data[0] = ["Alice", "30", "NYC"]
// data[1] = ["Bob", "25", "LA"]
// data[2] = ["Smith, J", "35", "Chicago"]

for (row in data) {
    print(row[0] + " is " + row[1] + " years old");
}

let rows = CSV.parse("1,2\n3,4\n", { header: false });
print(rows);  // [["1", "2"], ["3", "4"]]
```

> **Note**: Fields are always strings. Convert numbers yourself, for example with `Int.parse(row[1])`.

---
