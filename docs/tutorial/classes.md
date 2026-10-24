---
layout: page
footer: false
title: "Classes"
code: |
  class Stack {
      private items = [];

      fn push(item) {
          this.items.push(item);
      }

      // Add pop() and size().
  }

  let stack = Stack();
  stack.push("a");
  stack.push("b");
  print(stack.pop(), stack.size());   // b 1
solution: |
  class Stack {
      private items = [];

      fn push(item) {
          this.items.push(item);
      }

      fn pop() {
          return this.items.pop();
      }

      fn size(): Int {
          return this.items.length;
      }
  }

  let stack = Stack();
  stack.push("a");
  stack.push("b");
  print(stack.pop(), stack.size());   // b 1
---

<TutorialStep>

# Classes

A class groups data (fields) with the functions that use it (methods). `private` fields can only be used inside the class. Create an object by calling the class:

```pleno
class Temperature {
    private celsius: Float;

    constructor(celsius: Float) {
        this.celsius = celsius;
    }

    fn fahrenheit(): Float {
        return this.celsius * 9.0 / 5.0 + 32.0;
    }
}

print(Temperature(20.0).fahrenheit());   // 68
```

A class can `extend` another and reuse its methods, and `implements` an interface that lists the methods it must have.

## Your turn

Give `Stack` a `pop()` method that removes and returns the last item, and a `size()` method.

</TutorialStep>
