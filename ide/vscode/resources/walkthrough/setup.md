# Getting Started with Pleno

Welcome to Pleno! This walkthrough will help you set up your environment and run your first Pleno program.

## 1. Install the Toolchain

Ensure you have the Pleno toolchain installed on your system. You can build it from source or download a pre-built binary.

[Install Instructions](https://github.com/otabekoff/pleno#installation)

## 2. Initialize a New Project

Open a folder in VSCode and run the following command in your terminal:

```bash
pleno init my-project
```

Or use the Pleno: Project Overview sidebar to get started.

## 3. Your First Program

Create a file named `hello.pln` and add the following code:

```pleno
fun main() {
    print("Hello, Pleno!")
}
```

## 4. Run and Debug

Click the **Run** button in the top right corner or run `pleno run hello.pln` in your terminal.

Check the **Problems** tab for any syntax or type errors detected by the Pleno Language Server.

## 5. Explore the Ecosystem

Visit the [Documentation](https://github.com/otabekoff/pleno/docs) to learn more about the language features and standard library.
