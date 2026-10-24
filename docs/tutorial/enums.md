---
layout: page
footer: false
title: "Enums and match"
code: |
  enum Shape {
      Circle(radius),
      Triangle(base, height)
  }

  fn area(shape: Shape): Float {
      match (shape) {
          case Shape.Circle(r) => return 3.14159 * r * r,
          case _ => return 0.0
      }
  }

  print(area(Shape.Circle(1.0)));
  print(area(Shape.Triangle(4.0, 3.0)));   // should be 6
solution: |
  enum Shape {
      Circle(radius),
      Triangle(base, height)
  }

  fn area(shape: Shape): Float {
      match (shape) {
          case Shape.Circle(r) => return 3.14159 * r * r,
          case Shape.Triangle(b, h) => return b * h / 2.0
      }
  }

  print(area(Shape.Circle(1.0)));
  print(area(Shape.Triangle(4.0, 3.0)));   // should be 6
---

<TutorialStep>

# Enums and match

An enum lists the possible kinds of a value. Each kind can carry data, and `match` takes it apart:

```pleno
enum Payment {
    Cash,
    Card(last4),
    Transfer(bank, reference)
}

fn describe(payment: Payment): String {
    match (payment) {
        case Payment.Cash => return "cash",
        case Payment.Card(digits) => return `card ending ${digits}`,
        case Payment.Transfer(bank, _) => return `transfer from ${bank}`
    }
}
```

## Your turn

Finish `area` so it also handles `Shape.Triangle(base, height)`.

</TutorialStep>
