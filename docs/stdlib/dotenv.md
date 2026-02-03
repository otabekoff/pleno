# std:dotenv

Load environment variables from `.env` files.

## Import

```pleno
import { Dotenv } from "std:dotenv";
```

## Functions

### `config(path: String = ".env"): Void`

Loads environment variables from a file into the process environment.

```pleno
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

Dotenv.config();

let dbUrl = Process.env("DATABASE_URL");
let apiKey = Process.env("API_KEY");
```

---
