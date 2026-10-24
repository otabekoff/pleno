---
layout: page
footer: false
title: "Where to go next"
code: |
  class Task {
      public title: String;
      public done: Bool = false;

      constructor(title: String) {
          this.title = title;
      }
  }

  let tasks = ["Learn Pleno", "Write a project", "Share it"]
      |> map(fn(title) => Task(title));
  tasks[0].done = true;

  for (task in tasks) {
      let mark = task.done ? "x" : " ";
      print(`[${mark}] ${task.title}`);
  }

  let left = tasks |> filter(fn(t) => !t.done);
  print(`${left.length} to go`);
---

<TutorialStep>

# Where to go next

You have seen most of Pleno: values and variables, collections, functions, types, classes, enums, errors, pipelines, async code, modules and tests.

To keep going:

- [Install Pleno](/guide/installation) and set up the [VS Code extension](/guide/editor).
- Create a project with `plm init` and read about [packages](/guide/packages).
- Read the [language reference](/specs/) and the [standard library](/stdlib/).
- Try ideas in the [playground](/playground) and share them with a link.

The program on the right uses several lessons at once. Change it and see what happens.

</TutorialStep>
