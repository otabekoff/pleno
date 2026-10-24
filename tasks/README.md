# Tasks

How work on Pleno is tracked. Every open item says what exists and what is missing (its status), not only a checkbox. When an item is completely done, it is removed from its list and logged in `done.md`.

| File | What it holds |
| --- | --- |
| [`v1.md`](v1.md) | Everything Pleno needs before a stable 1.0 release, in priority order |
| [`later.md`](later.md) | Planned work after 1.0 |
| [`ideas.md`](ideas.md) | Ideas and open questions, not yet planned |
| [`done.md`](done.md) | What was finished, by date |

Reference material that is not a task list lives in [`notes/`](../notes/): the language specification, the memory-safety audit, release and signing notes.

## Workflow

1. Pick the top unfinished item in `v1.md`.
2. Open a GitHub issue for it, work on a branch, and open a pull request that closes the issue.
3. When it merges, remove the item and add a line to `done.md`. If only part is done, update the item's status instead.
