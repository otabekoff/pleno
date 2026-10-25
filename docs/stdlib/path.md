# std:path

> **Stability:** stable in 1.0.

Cross-platform path manipulation utilities.

## Import

Import the `Path` namespace, or import single functions:

```pleno
import { Path } from "std:path";
import { join, basename } from "std:path";

print(basename(join("docs", "intro.md")));  // intro.md
```

## Constants

### `separator`

The platform path separator: `"/"` on Linux and macOS, `"\"` on Windows.

```pleno
import { Path } from "std:path";

print(Path.separator);
```

## Functions

### `join(...parts: String): String`

Joins path segments using the platform separator.

```pleno
import { Path } from "std:path";

let fullPath = Path.join("home", "user", "docs");
print(fullPath);
// Linux: home/user/docs
// Windows: home\user\docs
```

### `dirname(path: String): String`

Returns the directory portion of a path. A bare file name gives `"."`.

```pleno
import { Path } from "std:path";

print(Path.dirname("/home/user/file.txt"));  // /home/user
print(Path.dirname("file.txt"));             // .
```

### `basename(path: String): String`

Returns the last component of a path.

```pleno
import { Path } from "std:path";

print(Path.basename("/home/user/file.txt"));  // file.txt
print(Path.basename("/home/user/"));          // user
```

### `extname(path: String): String`

Returns the file extension (including the dot), or `""` if there is none.

```pleno
import { Path } from "std:path";

print(Path.extname("file.txt"));        // .txt
print(Path.extname("archive.tar.gz"));  // .gz
print(Path.extname("noext") == "");     // true
```

### `exists(path: String): Bool`

Checks if a path exists on the filesystem.

```pleno
import { Path } from "std:path";

if (Path.exists("config.json")) {
    print("Config found");
} else {
    print("No config");
}
```

### `isFile(path: String): Bool`

Returns `true` if the path is a regular file.

```pleno
import { Path } from "std:path";
import { File, Directory } from "std:io";

File.write("main.pln", "print(1);");
Directory.create("src");
print(Path.isFile("main.pln"));  // true
print(Path.isFile("src"));       // false
File.delete("main.pln");
Directory.delete("src");
```

### `isDir(path: String): Bool`

Returns `true` if the path is a directory.

```pleno
import { Path } from "std:path";
import { File, Directory } from "std:io";

File.write("main.pln", "print(1);");
Directory.create("src");
print(Path.isDir("src"));       // true
print(Path.isDir("main.pln"));  // false
File.delete("main.pln");
Directory.delete("src");
```

### `resolve(path: String): String`

Resolves a path to an absolute path. Existing paths are resolved through the filesystem (symlinks are followed). Paths that don't exist are joined to the current working directory.

```pleno
import { Path } from "std:path";

print(Path.resolve("src/main.pln"));
// e.g. /home/user/project/src/main.pln
```

---
