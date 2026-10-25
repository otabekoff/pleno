# Pleno language specification

This file used to hold the full specification, with implemented, partial and future features mixed together. It has been split:

- **The 1.0 reference** is `docs/specs/` (published at https://pleno.mohirlab.uz/specs/). It describes only what Pleno implements, and every `pleno` code block in it is run by `scripts/check-doc-examples.py` in CI.
- **What 1.x guarantees** is `docs/specs/compatibility.md`.
- **The standard library and the stability of each module** are in `docs/stdlib/`, with the list in `docs/stdlib/index.md`.
- **Features that are designed but not implemented** are in `notes/v2-draft.md`, each with its status.

Change the language by changing `docs/specs/` (and its examples) together with the implementation; put designs for later in `notes/v2-draft.md`.
