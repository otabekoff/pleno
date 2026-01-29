# Pleno Examples

This directory contains example Pleno programs demonstrating various language features and best practices.

## Running Examples

```bash
# Run an example
cargo run --bin pleno-cli -- run examples/hello_world.pln

# Or if you have pleno installed
pleno run examples/hello_world.pln
```

## Available Examples

### Basic Examples

- **hello_world.pln** - Simple "Hello, World!" program
- **variables.pln** - Variable declarations and types
- **functions.pln** - Function definitions and calls
- **control_flow.pln** - If statements, loops, and match expressions

### Object-Oriented Programming

- **classes.pln** - Class definitions, constructors, and methods
- **inheritance.pln** - Class inheritance and method overriding
- **interfaces.pln** - Interface definitions and implementations

### Functional Programming

- **higher_order_functions.pln** - Functions as first-class values
- **closures.pln** - Closure examples and variable capture
- **pipeline.pln** - Pipeline operator usage

### Async Programming

- **async_basics.pln** - Basic async/await usage
- **promises.pln** - Promise creation and chaining
- **concurrent_tasks.pln** - Running multiple tasks concurrently

### Standard Library

- **file_io.pln** - File reading and writing
- **http_client.pln** - Making HTTP requests
- **json_parsing.pln** - JSON encoding and decoding
- **regex.pln** - Regular expression usage

### Advanced Features

- **generics.pln** - Generic types and functions
- **error_handling.pln** - Try-catch and error propagation
- **macros.pln** - Macro definitions and usage
- **decorators.pln** - Decorator usage

## Contributing Examples

When adding new examples:

1. Keep examples focused on a single concept
2. Add clear comments explaining the code
3. Include expected output in comments
4. Test that the example runs successfully
5. Update this README with the new example

## Example Template

```pleno
/// Example: [Brief description]
/// 
/// This example demonstrates [what it demonstrates].
/// 
/// Expected output:
/// [show expected output]

// Your code here
```

## Learning Path

Recommended order for beginners:

1. hello_world.pln
2. variables.pln
3. functions.pln
4. control_flow.pln
5. classes.pln
6. error_handling.pln
7. file_io.pln
8. async_basics.pln

---

For more information, see the [Pleno Language Specification](../specs.md).
