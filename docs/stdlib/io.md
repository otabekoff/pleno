# std:io

> **Stability:** stable in 1.0.

File and directory operations for reading, writing, and managing the filesystem.

## Import

```pleno
import { File, Directory } from "std:io";
```

## File

`File` holds functions for working with whole files.

### `read(path: String): String`

Reads the entire contents of a file as a string.

```pleno
import { File } from "std:io";

File.write("config.json", "{\"debug\": true}");
let content = File.read("config.json");
print(content);  // {"debug": true}
File.delete("config.json");
```

### `write(path: String, content: String): Void`

Writes content to a file, creating it if it doesn't exist or overwriting if it does.

```pleno
import { File } from "std:io";

File.write("output.txt", "Hello, Pleno!");
print(File.read("output.txt"));  // Hello, Pleno!
File.delete("output.txt");
```

### `append(path: String, content: String): Void`

Appends content to the end of a file. The file is created if it doesn't exist.

```pleno
import { File } from "std:io";

File.append("log.txt", "first entry\n");
File.append("log.txt", "second entry\n");
print(File.read("log.txt"));
File.delete("log.txt");
```

### `readLines(path: String): String[]`

Reads a file and returns its lines, without line endings.

```pleno
import { File } from "std:io";

File.write("names.txt", "Ada\nGrace\nLinus\n");
let lines = File.readLines("names.txt");
print(lines.length);  // 3
print(lines[1]);      // Grace
File.delete("names.txt");
```

### `exists(path: String): Bool`

Returns `true` if something exists at `path`.

```pleno
import { File } from "std:io";

if (File.exists("config.json")) {
    print(File.read("config.json"));
} else {
    print("no config");
}
```

### `delete(path: String): Void`

Deletes a file.

```pleno
import { File } from "std:io";

File.write("temp.txt", "scratch");
File.delete("temp.txt");
print(File.exists("temp.txt"));  // false
```

### `copy(src: String, dest: String): Void`

Copies a file from source to destination.

```pleno
import { File } from "std:io";

File.write("original.txt", "data");
File.copy("original.txt", "backup.txt");
print(File.read("backup.txt"));  // data
File.delete("original.txt");
File.delete("backup.txt");
```

### `move(src: String, dest: String): Void`

Moves or renames a file.

```pleno
import { File } from "std:io";

File.write("draft.txt", "text");
File.move("draft.txt", "final.txt");
print(File.exists("draft.txt"));  // false
print(File.read("final.txt"));    // text
File.delete("final.txt");
```

### `open(path: String, mode: String = "r"): File`

Opens a file and returns a file handle. `mode` is `"r"` (read), `"w"` (write, truncating the file) or `"a"` (append). The same function is also exported as `open`.

A handle has these methods:

- `write(text)` writes a string (write and append modes).
- `flush()` flushes buffered output.
- `close()` closes the handle.
- `read()` and `readLines()` return the contents (read mode).

You can also pass a handle to `print` with `file=`.

```pleno
import { File } from "std:io";

let out = File.open("notes.txt", "w");
out.write("line 1\n");
print("line 2", file=out);
out.close();

let input = File.open("notes.txt");
print(input.readLines());  // ["line 1", "line 2"]
File.delete("notes.txt");
```

---

## Standard streams

### `stdout(): File` and `stderr(): File`

Return handles for standard output and standard error.

```pleno
import { stdout, stderr } from "std:io";

let out = stdout();
out.write("to stdout\n");
print("to stderr", file=stderr());
```

---

## Directory

`Directory` holds functions for working with directories.

### `create(path: String): Void`

Creates a single directory. The parent directory must already exist.

```pleno
import { Directory } from "std:io";

Directory.create("output");
print(Directory.exists("output"));  // true
Directory.delete("output");
```

### `createRecursive(path: String): Void`

Creates a directory and any missing parent directories.

```pleno
import { Directory } from "std:io";

Directory.createRecursive("data/logs/2026");
print(Directory.exists("data/logs/2026"));  // true
Directory.deleteRecursive("data");
```

### `exists(path: String): Bool`

Returns `true` if `path` is a directory.

```pleno
import { Directory } from "std:io";

if (!Directory.exists("output")) {
    Directory.create("output");
}
Directory.delete("output");
```

### `list(path: String): String[]`

Lists the names of all entries (files and subdirectories) in a directory. The order is not guaranteed.

```pleno
import { File, Directory } from "std:io";

Directory.create("box");
File.write("box/a.txt", "a");
Directory.create("box/sub");
for (entry in Directory.list("box")) {
    print(entry);
}
Directory.deleteRecursive("box");
```

### `listFiles(path: String): String[]`

Lists only the files in a directory.

### `listDirectories(path: String): String[]`

Lists only the subdirectories of a directory.

`Directory.list_files` and `Directory.list_directories` are older spellings of the same two functions. They are not covered by the [1.x compatibility promise](../specs/compatibility.md) and may be removed in 2.0; use `listFiles` and `listDirectories`.

```pleno
import { File, Directory } from "std:io";

Directory.create("box");
File.write("box/a.txt", "a");
Directory.create("box/sub");
print(Directory.listFiles("box"));        // ["a.txt"]
print(Directory.listDirectories("box"));  // ["sub"]
Directory.deleteRecursive("box");
```

### `delete(path: String): Void`

Deletes an empty directory.

```pleno
import { Directory } from "std:io";

Directory.create("temp");
Directory.delete("temp");
print(Directory.exists("temp"));  // false
```

### `deleteRecursive(path: String): Void`

Deletes a directory and everything inside it.

```pleno
import { File, Directory } from "std:io";

Directory.createRecursive("cache/images");
File.write("cache/images/a.png", "");
Directory.deleteRecursive("cache");
print(Directory.exists("cache"));  // false
```

---
