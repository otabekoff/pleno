# std:csv

CSV parsing.

## Import

```pleno
import { CSV } from "std:csv";
```

## Functions

### `parse(csv: String): Array<Array<String>>`

Parses a CSV string into a 2D array of strings.

```pleno
let data = CSV.parse(`
name,age,city
Alice,30,NYC
Bob,25,LA
Charlie,35,Chicago
`);

// data[0] = ["name", "age", "city"]  (header)
// data[1] = ["Alice", "30", "NYC"]
// data[2] = ["Bob", "25", "LA"]

for (row in data) {
    print(row[0] + " is " + row[1] + " years old");
}
```

> **Note**: The first row is typically the header. Parse accordingly in your application logic.

---
