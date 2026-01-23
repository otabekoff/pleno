# Bytecode VM Design (Register-Based)

Pleno will use a register-based bytecode VM from the start. This choice is
performance-oriented and aligns with the goal of being faster than Python while
remaining dynamic. A register VM yields fewer instructions, fewer memory
push/pop operations, and a cleaner path to JIT later.

## High-Level Pipeline

```
Source (.pln)
  -> Lexer -> Parser -> AST
  -> Compiler -> Bytecode (Chunk)
  -> VM -> Execute
```

## Instruction Set (v0)

Design goals:
- Keep opcodes minimal and orthogonal.
- Use fixed-width instruction encoding for fast decoding.
- Keep register count bounded per function for locality.

### Encoding

Each instruction is 4 bytes:

```
[Opcode: u8][A: u8][B: u8][C: u8]
```

- `A` is destination register.
- `B`, `C` are source registers or immediate indices (depending on opcode).

### Core Opcodes

#### Register and Constants
- `Move`           (R[A] = R[B])
- `LoadConst`      (R[A] = Constants[B])
- `LoadNull`       (R[A] = null)
- `LoadTrue`       (R[A] = true)
- `LoadFalse`      (R[A] = false)

#### Arithmetic
- `Add` `Sub` `Mul` `Div` `Mod`
- `Neg`

#### Comparison
- `Eq` `Ne` `Lt` `Le` `Gt` `Ge`

#### Logical
- `Not`
- `And` `Or` (compiled via jumps for short-circuit)

#### Control Flow
- `Jump offset`
- `JumpIf offset`    (if R[A] then PC += offset)
- `JumpIfNot offset` (if !R[A] then PC += offset)

#### Variables
- `GetGlobal` (R[A] = Globals[Constants[B]])
- `SetGlobal` (Globals[Constants[B]] = R[A])

#### Calls
- `Call`    (R[A] = R[B](R[C]..R[C+N]))
- `Return`  (return R[A])
- `Closure` (R[A] = closure(proto[B]))

#### Objects and Collections
- `GetProperty` (R[A] = R[B][Constants[C]])
- `SetProperty` (R[B][Constants[C]] = R[A])
- `GetIndex`    (R[A] = R[B][R[C]])
- `SetIndex`    (R[B][R[C]] = R[A])
- `NewArray`    (R[A] = [])
- `NewMap`      (R[A] = {})
- `ArrayAppend` (R[A].push(R[B]))

#### Advanced
- `Import`   (R[A] = import(Constants[B]))
- `Throw`    (throw R[A])
- `TypeOf`   (R[A] = typeof(R[B]))
- `Is`       (R[A] = R[B] is Type[C])

#### Fast Paths (Optional v0.1)
- `LoadInt`  (R[A] = smallint(B))
- `AddInt`   (R[A] = R[B] + immediate)
- `SubInt`   (R[A] = R[B] - immediate)

## Bytecode Structures

- `Chunk.code: Vec<Instruction>`
- `Chunk.constants: Vec<Value>`
- `Chunk.prototypes: Vec<Function>`
- `Chunk.lines: Vec<u32>`
- `Chunk.max_registers: usize`

## Compiler Guide

### Register Allocation

- Per function, allocate registers for locals and temporaries.
- Keep registers small (<= 256) for compact encoding.
- Free temporaries after use to reduce pressure.

### Expressions

- **Literal**: `LoadConst` into a fresh register.
- **Binary**: compile left into R1, right into R2, emit opcode into R3.
- **Unary**: compile operand into R1, emit opcode into R2.

### Variables

- `let x = expr` -> compile expr into Rn, bind local to Rn.
- `x = expr` -> compile expr into Rn, emit store to local/global slot.

### If/Else

1. Compile condition into Rn.
2. `JumpIfNot` to else label.
3. Compile then block.
4. `Jump` to end label.
5. Patch else label, compile else block.

### While / For

- Track loop start and end labels.
- `JumpIfNot` exits loop.
- `Jump`/`Loop` (negative offset) for continue.

### Functions + Calls

- Each function produces a `Chunk` with its own register file size.
- `Call` uses a base register and contiguous args.

### Closures

- Capture locals into upvalues in the compiler.
- Emit `Closure` with upvalue descriptors.
- Runtime resolves captures to shared cells.

### OOP

- `class` compiles to class object in constants.
- `obj.field` -> `GetProperty name_idx`.
- `obj.field = expr` -> compile expr, `SetProperty`.
- `obj.method(a,b)` -> `Invoke` (optional), or `GetProperty` + `Call`.

### Pattern Matching

Compile match to a cascade of tests and jumps.

- Evaluate match value once into a temp register.
- For each case: emit test, `JumpIfNot` to next.
- Guard expressions compiled into boolean checks.
- Default case is the final fall-through.

## VM Implementation Guide

### VM State

- `registers: Vec<Value>`
- `frames: Vec<CallFrame>`
- `globals: HashMap<String, Value>`

### Call Frame

- `closure: Rc<Closure>`
- `ip: usize`
- `base: usize` (register base for frame)

### Execution Loop

- Fetch `Instruction` at `ip` and increment.
- Decode `opcode, a, b, c`.
- Dispatch via `match`.
- For errors, return `RuntimeError` (no panics).

## Value Model (Dynamic)

Start with a tagged enum; move to NaN boxing later if needed.

```
Value::Null | Bool | Int | Float | String | Array | Map | Set | Function | Closure | Object | Class
```

## Garbage Collection Design

### v0: Tracing mark-sweep

- **Heap** stores arrays, maps, strings, functions, objects.
- **Roots**:
  - Register file
  - Globals
  - Call frames (closures, upvalues)
- **Mark**: traverse references and mark reachable.
- **Sweep**: free unmarked objects.

### Future: Generational GC

- Young gen for short-lived values.
- Old gen for long-lived values.
- Requires write barriers.

## Why Register VM (Best Now)

- Fewer instructions per program.
- Better cache locality vs. stack VM.
- Easier path to JIT and inline caching.

We can start with register VM and still add optimizations incrementally:
- Inline caches for property access
- NaN boxing for compact values
- Specialized opcodes for common fast paths
