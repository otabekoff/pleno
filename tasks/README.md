Don't continue with this folder and files, this is for V2.

A dedicated `tasks/` folder is actually a very solid habit, especially for long-living projects and solo dev workflows. Let’s turn it into a **simple but scalable task system** that feels like a lightweight Jira + roadmap, but just Markdown.

I’ll give you:

1. A clean folder structure
2. The exact Markdown files to create
3. What each file is for
4. How the workflow actually works day to day

No overengineering, but future-proof.

---

## 1. Recommended `tasks/` structure

```
tasks/
├── README.md
├── backlog.md
├── todo.md
├── in-progress.md
├── blocked.md
├── done.md
├── skipped.md
├── ideas.md
├── roadmap.md
└── sprint-2026-01.md
```

This is basically **Kanban + Roadmap + History**.

---

## 2. Core files (must-have)

### 1. `tasks/README.md` (the brain of the system)

Explains how *you* manage tasks in this project.

Example:

```markdown
# Task Management System

This folder contains all project tasks.

Workflow:
ideas → backlog → todo → in-progress → done

Special states:
- blocked: waiting for something
- skipped: consciously dropped

Sprints:
Each sprint has its own file: sprint-YYYY-MM.md
```

This is for **future you**.

---

### 2. `backlog.md` (everything that might be done)

Long-term parking lot.

```markdown
# Backlog

## Features
- [ ] Add plugin system
- [ ] Support config files
- [ ] Multi-language docs

## Tech Debt
- [ ] Refactor parser
- [ ] Improve error messages

## Research
- [ ] Compare LLVM vs Cranelift
```

Nothing here is urgent.

---

### 3. `todo.md` (next actionable tasks)

This is your **real daily driver**.

```markdown
# To Do

## High priority
- [ ] Implement AST visitor
- [ ] Write tests for lexer

## Medium
- [ ] Update README
- [ ] Add logging
```

Rule:
If it’s in `todo.md`, you **intend to work on it soon**.

---

### 4. `in-progress.md`

Only what you’re *currently touching*.

```markdown
# In Progress

- [ ] Implement bytecode VM
- [ ] Fix memory leak in GC
```

Golden rule:
**Never more than 3–5 items here.**

---

### 5. `done.md` (history & motivation)

```markdown
# Done

## 2026-01-23
- [x] Setup project structure
- [x] Add basic CLI

## 2026-01-20
- [x] Initial parser
```

This becomes your **project diary**.

Very powerful psychologically.

---

## 3. Control files (important but optional)

### `blocked.md`

When something is stuck.

```markdown
# Blocked

- [ ] Release build (waiting for CI fix)
- [ ] Plugin API (needs design decision)
```

Removes mental noise.

---

### `skipped.md` (underrated but elite)

This is **anti-scope-creep**.

```markdown
# Skipped / Dropped

- [ ] Build mobile app (not core goal)
- [ ] Rewrite in Rust (overkill)
```

This saves you from rethinking the same ideas again.

---

### `ideas.md`

Raw, messy brain dump.

```markdown
# Ideas

- AI code assistant
- Visual debugger
- Interactive REPL UI
```

No structure. Just capture.

---

## 4. Planning files

### `roadmap.md` (big picture)

This is product-level.

```markdown
# Roadmap

## v0.1
- Basic language syntax
- CLI runner

## v0.2
- VM
- Standard library

## v1.0
- Package manager
- Docs site
```

Think in **versions, not tasks**.

---

### Sprint file (very powerful)

Example: `sprint-2026-01.md`

```markdown
# Sprint 2026-01

Goal: Get minimal working interpreter

## Committed
- [ ] Lexer
- [ ] Parser
- [ ] AST

## Stretch
- [ ] REPL
- [ ] Error reporting
```

This is your **focus contract with yourself**.

---

## 5. How the workflow actually works

This is the real magic.

### Normal flow

```
ideas.md
   ↓
backlog.md
   ↓
todo.md
   ↓
in-progress.md
   ↓
done.md
```

### Daily routine (5 minutes)

1. Open `todo.md`
2. Move 1–3 items → `in-progress.md`
3. Work
4. When finished → move to `done.md`

---

## 6. Pro-level Markdown patterns

### Task with metadata

```markdown
- [ ] Implement GC
  - priority: high
  - estimate: 3h
  - depends: memory model
```

### Link tasks to code

```markdown
- [ ] Parser refactor (`src/parser.rs`)
```

### Decision log inside task

```markdown
- [ ] VM design
  - decision: stack-based, not register-based
  - reason: simpler implementation
```

Now your tasks become **engineering documentation**.

---

## 7. Why this system works (mentally)

This structure gives you:

| Problem          | Solved by   |
| ---------------- | ----------- |
| Overwhelm        | backlog     |
| Lack of focus    | in-progress |
| Scope creep      | skipped     |
| No direction     | roadmap     |
| Demotivation     | done        |
| Forgetting ideas | ideas       |
| Bad planning     | sprint      |

It’s basically **external brain for developers**.

---

## 8. Minimal version (if you want ultra-simple)

If you want the 20% version:

```
tasks/
├── todo.md
├── in-progress.md
├── done.md
└── backlog.md
```

That alone already beats 90% of dev workflows.

---

## Final advice (from experience)

For someone like you (building languages, frameworks, big ideas):

> Treat `tasks/` as seriously as `src/`.
