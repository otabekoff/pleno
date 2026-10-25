# Run and debug

Open a `.pln` file, for example:

```pleno
fn greet(name: String): String {
    return `Hello, ${name}!`;
}

print(greet("Pleno"));
```

- **Run File**, the play button in the editor title bar, saves the file and runs `pleno run` in
  the terminal.
- **Debug File**, or `F5`, starts the debugger. Click left of a line number to set a breakpoint,
  then step through the code and look at the variables.
- Errors and lint warnings show up in the **Problems** panel as you type.
- **Pleno: Open REPL** starts an interactive session to try code out.
