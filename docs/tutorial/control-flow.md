---
layout: page
footer: false
title: "Making decisions"
code: |
  for (n in 1..=15) {
      // Print Fizz, Buzz, FizzBuzz or n.
      print(n);
  }
solution: |
  for (n in 1..=15) {
      if (n % 15 == 0) {
          print("FizzBuzz");
      } else if (n % 3 == 0) {
          print("Fizz");
      } else if (n % 5 == 0) {
          print("Buzz");
      } else {
          print(n);
      }
  }
---

<TutorialStep>

# Making decisions

`if` and `else` choose between branches, and `while` repeats while a condition holds:

```pleno
let temperature = 23;
if (temperature > 25) {
    print("hot");
} else if (temperature > 15) {
    print("pleasant");
} else {
    print("cold");
}
```

`match` compares a value against several patterns, and `_` matches anything:

```pleno
let day = "Sat";
let kind = match (day) {
    case "Sat", "Sun" => "weekend",
    case _ => "weekday"
};
```

## Your turn

Print FizzBuzz for 1 to 15: `Fizz` for multiples of 3, `Buzz` for multiples of 5, `FizzBuzz` for both, and the number otherwise.

</TutorialStep>
