# std:csv

CSV parsing.

## Import

```pleno
import { CSV } from "std:csv";
```

`parse` can also be imported directly.

## Functions

### `parse(csv: String): Array<Array<String>>`

Parses a CSV string into an array of rows. Each row is an array of strings. Quoted fields may contain commas.

The first row is treated as the header and is not included in the result. Rows with a different number of fields than the header throw an error.

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
```

> **Note**: Fields are always strings. Convert numbers yourself, for example with `Int.parse(row[1])`.

---
