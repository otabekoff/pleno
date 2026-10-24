---
layout: page
footer: false
title: "Arrays, maps and ranges"
code: |
  let scores = {"Ada": 92, "Alan": 85, "Grace": 97};
  let mut total = 0;

  // Loop over scores.values() and add each one to total.

  print("total:", total);
solution: |
  let scores = {"Ada": 92, "Alan": 85, "Grace": 97};
  let mut total = 0;

  for (score in scores.values()) {
      total += score;
  }

  print("total:", total);
---

<TutorialStep>

# Arrays, maps and ranges

An **array** is an ordered list, a **map** pairs keys with values, and a **range** counts from one number to another:

```pleno
let colors = ["red", "magenta", "violet"];
colors.push("blue");
print(colors[0], colors.length);

let ages = {"Ada": 36, "Alan": 41};
ages["Grace"] = 85;
print(ages["Ada"], ages.keys());

print((1..=5).toArray());   // 1 to 5; 1..5 stops before 5
```

`for (item in collection)` visits each element.

## Your turn

Add up the scores in the map and print the total.

</TutorialStep>
