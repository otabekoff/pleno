## Priority: Standard Library & GC Integration


---


publishing with certificates not to consided the programming language as virus and installing safely on mac, windows and linux.

---

Debug mode, production mode logs for development.
Remove debug logs.

---


Code Runner extension should support for Pleno programming language.

---

To release production ready v1, what should we do?

Now, as we have finished v1 of the Pleno programming langguage, I want you to fully think and write, plan the next v2 of Pleno programming language. We need V2 roadmap, v2-specs.md. Here we'll add better features, make the old and inconsistent things enhanced, better, implement new features and support many things, more stdlibs. and many more. Think more and write detailed, Things will be added, upgraded, updated, fixed, supported.

Now, as we have finished v1 of the Pleno programming language, we fully support specs.md and many other things not listed in it too. Now what I want is, you need to add those already implemented, but missed things in specs.md to be noted.

Should we left unprefixed stuffs? or prefixed stuffs of variables and function names as is or should we use it or are they planned for later? Let's reconsider this and review each of them.

Work on each crate and make the working and complete and doing their job fully.

---


LSP and IDE features. REPL, also colors?.

More comment types?

V1 missing things noti implemented, deferred, technical debts should be identifed and should be saved to one file. v1-last-tasks.md

Scripts, management tools, makefile better vesrion. Organize the project.

Move to new version of taks management in tasks folder. New workflow.

More examples. Benchmark. IDE Features.

We've to think about bad practices, inconsistencies. We've to review what we have already and reconsider everything. Need to find inconsistencies, duplicated, bad dx and practices and fix and solve and upgrade, update them.

We've to think about better github management, better actions, better commits, better tools of github, better branch management.

New folder called prompts.

V2, we'll be the most ideal one.

Add more licenses from rust like githuib stuff.

I heard somewhere, making everything Boxed or boexed expressions in programming language is not good way and it makes slow. Is it true? Why? How can we avoid it? Is it possible to avoid it? How can we make it better? Do we have such cases or not? If we have such places, do we need to make them better and avoid or are there the places where they should be like so?

See `TECHNICAL_DEBT.md` for the full list of technical debt items. Update it. Now we need to search the project, the comments, and find all leakages, unimplemented stuffs, and write all the technical debts in technical debt file.

Clean the project sources folder.
make the project clean and organized. Of course mainly project root. And review README.md and make it a programming language README for open source.

Review CODE_OF_CONDUCT, CONTRIBUTING< LICENSE, SECURITY markdown fiels too. We need ARCHITECTURE and such files too.

Also update README.dev.md.

Let's have new roadmap for V2. 

Let's review whole the docs and make it great.
We have to deploy v1 fully. Now we need to update the docs.



To earn at a stage where we can write compiler of Pleno with itself, what should we do?

Run becnchmarks.

Expand test suite and error messages

Continue with the implementation/fix/bugfix/update of next tasks in TECHNICAL_DEBT.md and ROADMAP.md and if you need any info , specs.md file is ready for you. And make sure to update the files at the end of the jobs those are done. And if completely done tasks and if we don't address them anymore, remove them from ROADMAP.md and TECHNICAL_DEBT.md.

Ok continue with @SPECS_LOG.md  and if you need you can use @specs.md  for reference



How we can optimize the code?

How we can make Pleno very fast even if it is dynamic?

Does each crate finished its work fully and complete? I mean, any crate may not be giving errors or even warnings and working minimalistically, but not to be complete yet supporitng everything needed for v1, not compatible with specs and not including everthing needed, so identify limitations and the blocks and make their list and begin working on them.

AI can do the things, and make the project work finally without errors. But while developing, it can make so many mistakes with logic. Many beginner codes, no optimization, and very needless codes too. So, we've to avoid them and make the project code professional and written with best practices.  AI does children like coding sometimes, sometimes like professionals and sometimes like juniors or interns or someone who doesn't know coding. So, let's find those kind of unprofessional parts and enhance.

Via Claude AI generate V2 specs. Upgrades/updates/news/critical and breakign changes, new features, inclrementals, bugfixing, avoiding inconsistencies and more. Instead of fixing the real problems, or being lazy to implement and refactor, AI just might wanted to lie or be lazy or be tricky and just satisfied error and warning linters and compiler. So, let's find them and actually fix those parts.

AI model's generation exceeded the maximum output token limit like error might happen and couldn't do some works, or execution tool may have failed in some cases and some changes might not have been applied.



Add benching, reporting. Pleno vs Python vs JS vs TS vs Java vs C++ vs C vs Rust vs Kotlin. Speed and other tests? Builtin time tracking tests vs outer command execution time tests. Languages' inner time trackers may differ, so we need some command prompt/terminal tool?

# QUESTIONS
- To reach self-hosted compiler programming level what we need to do?
- To make this programming language very fast what we need to do?
- To make this programming language very stable what we need to do?
- To make this programming language complete what we need to do?
- To make this programming language all platforms supported and very good tooling what we need to do?

Now it is time to renew/fefresh the specs.md with what our programming language has and can do.

---

# A real type-safe dynamic language

| Feature                 | Why                          |
| ----------------------- | ---------------------------- |
| Dynamic                 | No type annotations required |
| Type-safe               | No weird JS/Python bugs      |
| JIT possible            | Types known at runtime       |
| AOT possible            | Type profiles                |
| IDE inference           | Runtime → static info        |
| Zero undefined behavior | Unlike C, Python, JS         |

You can even do:

x = 5
x = "hello"


But:

x + 3   // ❌ runtime type error


Instead of corrupting memory or returning garbage.

🧠 The correct modern model (2026)

The winning design is:

Gradually-Typed, Runtime-Verified, Type-Safe Dynamic Language

Which means:

• Types exist
• Types are checked
• But they are not mandatory to write

This is what Python tried with typing but without runtime enforcement.
This is what our language can do correctly.