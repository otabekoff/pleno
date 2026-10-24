# std:dotenv

Load environment variables from `.env` files.

## Import

```pleno
import { Dotenv } from "std:dotenv";
```

## Functions

### `config(path: String = ".env"): Void`

Reads `KEY=value` lines from a file and sets them in the process environment. Read the values with `Process.env` from [std:process](./process.md).

- Empty lines and lines starting with `#` are skipped.
- Spaces around the key and the value are trimmed. Quotes are kept as part of the value.
- Existing variables with the same name are overwritten.
- If the file does not exist, nothing happens and no error is raised.

```pleno
import { Dotenv } from "std:dotenv";

Dotenv.config();  // Loads from .env

// Or specify a path
Dotenv.config(".env.production");
```

**File Format:**

```
# Comments start with #
DATABASE_URL=postgres://localhost/mydb
API_KEY=secret-key-123
DEBUG=true
```

**Usage:**

```pleno
import { Dotenv } from "std:dotenv";
import { Process } from "std:process";
import { File } from "std:io";

File.write(".env", "DATABASE_URL=postgres://localhost/mydb\nAPI_KEY=secret-key-123\n");
Dotenv.config();

let dbUrl = Process.env("DATABASE_URL");
let apiKey = Process.env("API_KEY");
print(dbUrl);  // postgres://localhost/mydb
File.delete(".env");
```

---
