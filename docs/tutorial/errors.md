---
layout: page
footer: false
title: "Errors"
code: |
  fn parsePort(text: String): Int {
      let port = text.toInt();
      if (port < 1 || port > 65535) {
          throw Error(`${text} is not a valid port`);
      }
      return port;
  }

  print(parsePort("8080"));
  print(parsePort("70000"));
solution: |
  fn parsePort(text: String): Int {
      let port = text.toInt();
      if (port < 1 || port > 65535) {
          throw Error(`${text} is not a valid port`);
      }
      return port;
  }

  print(parsePort("8080"));
  try {
      print(parsePort("70000"));
  } catch (e) {
      print("Invalid:", e.message);
  }
---

<TutorialStep>

# Errors

`throw` stops the current function with an error, and `try`/`catch` handles it. Errors carry a `message`:

```pleno
fn withdraw(balance: Float, amount: Float): Float {
    if (amount > balance) {
        throw Error("not enough money");
    }
    return balance - amount;
}

try {
    withdraw(10.0, 50.0);
} catch (e) {
    print("Refused:", e.message);
}
```

You can define your own error classes with `class NotFound extends Error {}`, and catch them by type: `catch (e: NotFound)`. A `finally` block always runs.

## Your turn

`parsePort` throws for invalid ports. Catch the error and print `Invalid: ...` instead of letting the program stop.

</TutorialStep>
