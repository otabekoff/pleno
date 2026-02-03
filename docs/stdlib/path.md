# std:path

Cross-platform path manipulation utilities.

## Import

```pleno
import { Path } from "std:path";
```

## Functions

### `join(...parts: String): String`

Joins path segments using the platform separator.

```pleno
let fullPath = Path.join("home", "user", "docs");
// Linux: "home/user/docs"
// Windows: "home\\user\\docs"
```

### `dirname(path: String): String`

Returns the directory portion of a path.

```pleno
Path.dirname("/home/user/file.txt");  // "/home/user"
Path.dirname("file.txt");              // "."
```

### `basename(path: String): String`

Returns the filename portion of a path.

```pleno
Path.basename("/home/user/file.txt");  // "file.txt"
Path.basename("/home/user/");          // "user"
```

### `extname(path: String): String`

Returns the file extension (including the dot).

```pleno
Path.extname("file.txt");     // ".txt"
Path.extname("archive.tar.gz"); // ".gz"
Path.extname("noext");        // ""
```

### `exists(path: String): Bool`

Checks if a path exists on the filesystem.

```pleno
if (Path.exists("config.json")) {
    print("Config found");
}
```

### `isFile(path: String): Bool`

Returns `true` if the path is a regular file.

```pleno
Path.isFile("main.pln");  // true
Path.isFile("src");       // false (directory)
```

### `isDir(path: String): Bool`

Returns `true` if the path is a directory.

```pleno
Path.isDir("src");        // true
Path.isDir("main.pln");   // false (file)
```

### `resolve(path: String): String`

Resolves a path to an absolute path.

```pleno
Path.resolve("./src/main.pln");
// "/home/user/project/src/main.pln"
```

---
