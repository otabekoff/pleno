---
layout: page
footer: false
title: "Async code"
code: |
  let tasks = [
      spawn(fn() => "Pleno"),
      // Add two more tasks.
  ];

  let words = await Promise.all(tasks);
  print(words.join(" "));
solution: |
  let tasks = [
      spawn(fn() => "Pleno"),
      spawn(fn() => "runs"),
      spawn(fn() => "concurrently")
  ];

  let words = await Promise.all(tasks);
  print(words.join(" "));
---

<TutorialStep>

# Async code

`spawn` runs a function as a separate task and returns a promise; `await` waits for its result. Functions marked `async` can `await` inside them:

```pleno
let answer = spawn(fn() => 6 * 7);
print(await answer);   // 42
```

A **channel** passes values between tasks. `for (value in channel)` waits for each value and stops when the channel is closed:

```pleno
import { Channel } from "std:concurrency";

let numbers = Channel(10);
spawn(fn() {
    numbers.send(1);
    numbers.send(2);
    numbers.close();
});
for (n in numbers) {
    print(n);
}
```

## Your turn

Start three tasks that each return a word, then wait for all of them with `Promise.all` and print the joined sentence.

</TutorialStep>
