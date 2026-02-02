# Pleno Architecture

This document describes the high-level architecture of the Pleno programming language.

## 🏗️ System Overview

Pleno is a stack-based virtual machine with a multi-stage compilation pipeline. It is designed for safety, performance, and modern language features like async/await and true encapsulation.

```mermaid
graph TD
    Source[Source Code .pln] --> Lexer
    Lexer --> Tokens[Token Stream]
    Tokens --> Parser
    Parser --> AST[Abstract Syntax Tree]
    AST --> Compiler
    Compiler --> Bytecode[Opcode Stream]
    Bytecode --> VM[Virtual Machine]
    VM --> Native[Runtime/Stdlib]
```

## 1. Compilation Pipeline

### Lexical Analysis (`pleno-lexer`)

The lexer converts raw UTF-8 source code into a stream of `Token`s. It handles:

- **String Interpolation**: Complex expressions within `${}` inside strings.
- **Comments**: Tracking line, block, and documentation comments.
- **Spans**: Recording start and end offsets for diagnostics.

### Parsing (`pleno-parser`)

A hand-written recursive descent parser that transforms tokens into an **AST**.

- **Gradual Typing**: Supports optional type annotations and union types.
- **Modern Syntax**: Implements classes, interfaces, match expressions, and async syntax.
- **DCE (Dead Code Elimination)**: Basic static analysis to remove unreachable blocks during parsing where possible.

### Compilation (`pleno-compiler`)

The compiler transforms the AST into a `Chunk` of bytecode instructions.

- **Register Allocation**: Maps variables to local registers.
- **Closure Analysis**: Identifies captured variables and generates `Opcode::Capture` instructions.
- **Constant Folding**: Pre-calculates constant expressions to reduce runtime overhead.

## 2. Virtual Machine (`pleno-vm`)

Pleno uses a **register-based** stack machine. Unlike stack machines that push/pop every operand (like JVM), Pleno typically operates on fixed register indices within a call frame.

### Instruction Format

Each `Instruction` is a packed 32-bit word:

- **Opcode** (8 bits): The operational command.
- **A** (8 bits): Usually the destination register or primary operand.
- **B** (8 bits): Primary source register or constant index.
- **C** (8 bits): Secondary source register or extra flag.

### Call Stack

Each function call pushes a `CallFrame`:

- **Function Index**: Index into the current chunk's function list.
- **Return Address**: Instruction pointer to resume after return.
- **Base Register**: Offset into the global register stack where this frame starts.
- **Closure State**: If the function is a closure, it holds the captured variables.

### Concurrency Model

Pleno implements cooperative multitasking via an internal **Event Loop**.

- **Promises**: First-class objects representing asynchronous results.
- **AsyncTask**: Suspended VM states that are resumed when their promise is resolved (e.g., IO completion, Mutex unlock).
- **Spawn**: Spawns a new task in the event loop, allowing concurrently executing code.

## 3. Value System (`pleno-value`)

### Value Representation

The `Value` enum is the heart of the system. It uses `Rc` (Reference Counting) or `GcRc` (Garbage Collected Reference Counting) for complex types:

- **Primitives**: `Int` (i64), `Float` (f64), `Bool`, `Char`, `Null`.
- **Heap Types**: `String`, `Array`, `Map`, `Object`, `Function`.
- **Special**: `Promise`, `Cell` (for mutable captures), `BoundMethod`.

### Encapsulation

Pleno enforces true encapsulation at the VM level.

- **Visibility**: `private`, `protected`, `public`, and `internal` modifiers.
- **Access Control**: The VM checks the current `CallFrame`'s class and module against the target property's visibility before allowing access.

## 4. Garbage Collection (`pleno-gc`)

Pleno uses a hybrid memory management strategy:

1. **Reference Counting**: Immediate cleanup for most objects.
2. **Cycle Detection**: A mark-and-sweep collector that periodically clears cyclic references which `Rc` cannot handle.

## 5. Standard Library & Native Interface

Pleno bridges to Rust via **Native Functions**. The `pleno-stdlib` crate provides:

- **IO**: File system and terminal operations.
- **Net**: HTTP client and server capabilities.
- **Concurrency**: `Mutex` and `Channel` primitives.
- **Time**: High-precision timers and DateTime utilities.

---

For more details on specific components, see the developer guide in [README.dev.md](README.dev.md).
