---
title: "Toward 1.0: making Pleno dependable"
date: 2026-10-24
author: Otabek
description: Every documentation example runs now, undefined names and failing tests are errors, and the runtime handles numbers safely.
---

This month's work had one goal: when the documentation says something works, it should work, and when something is wrong, Pleno should say so clearly instead of carrying on.

## Every example now runs

We ran every one of the 308 code blocks in this documentation against the current implementation. Forty-one did not even parse. Most were real gaps in Pleno, so we fixed the language rather than the docs:

- Enum type annotations (`let c: Color = Color.Red`) failed for every enum.
- `x |> f(a)` called `f(a)` first instead of `f(x, a)`.
- Iterating a channel never ended.
- A skipped typed `catch` replaced the error seen by the next branch.
- Fourteen documented import names did not exist.

Features the docs describe but Pleno does not have yet, such as destructuring, are now marked **Planned**.

## Errors you can see

- Reading a name that was never defined used to give `null`. Now it is an error, and `pleno check` reports misspelled names before you run anything.
- A failing test inside `std:test` used to print a cross while the file still passed. Now the run fails.
- An async task that failed only printed a warning. Now the program ends with the error.

## Safer runtime

Infinity and NaN, which used to decode as `null`, are real numbers. Integers keep their full 64 bits, division and modulo by zero are errors, and overflow is reported instead of crashing. Fuzzing runs on every pull request and has already found a compiler crash, now fixed.

## Tools

The VS Code extension can install Pleno for you (**Pleno: Install or Update Toolchain**). The terminal output got colors and the Pleno gradient. `plm init` creates projects whose tests actually run.

## What is left for 1.0

The honest list is in [tasks/v1.md](https://github.com/otabekoff/pleno/blob/main/tasks/v1.md). The biggest items: a frozen 1.0 specification, speed (Pleno is slower than CPython today), destructuring, and signed releases you can install from a package manager.
