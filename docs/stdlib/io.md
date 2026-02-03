# std:io

File and directory operations for reading, writing, and managing the filesystem.

## Import

```pleno
import { File, Directory } from "std:io";
```

## File

The `File` class provides static methods for file operations.

### `read(path: String): String`

Reads the entire contents of a file as a string.

```pleno
let content = File.read("config.json");
print(content);
```

### `write(path: String, content: String): Void`

Writes content to a file, creating it if it doesn't exist or overwriting if it does.

```pleno
File.write("output.txt", "Hello, Pleno!");
```

### `append(path: String, content: String): Void`

Appends content to the end of a file.

```pleno
File.append("log.txt", "New entry\n");
```

### `exists(path: String): Bool`

Returns `true` if the file exists.

```pleno
if (File.exists("config.json")) {
    let config = File.read("config.json");
}
```

### `delete(path: String): Void`

Deletes a file.

```pleno
File.delete("temp.txt");
```

### `copy(src: String, dest: String): Void`

Copies a file from source to destination.

```pleno
File.copy("original.txt", "backup.txt");
```

---

## Directory

The `Directory` class provides static methods for directory operations.

### `create(path: String): Void`

Creates a directory (including parent directories if needed).

```pleno
Directory.create("data/logs/2026");
```

### `exists(path: String): Bool`

Returns `true` if the directory exists.

```pleno
if (!Directory.exists("output")) {
    Directory.create("output");
}
```

### `list(path: String): String[]`

Lists all entries (files and subdirectories) in a directory.

```pleno
let entries = Directory.list(".");
for (entry in entries) {
    print(entry);
}
```

### `delete(path: String): Void`

Deletes an empty directory.

```pleno
Directory.delete("temp");
```

---
