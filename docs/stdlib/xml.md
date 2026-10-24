# std:xml

XML parsing.

## Import

```pleno
import { XML } from "std:xml";
```

`parse` can also be imported directly.

## Functions

### `parse(xml: String): Map`

Parses an XML string and returns its root element as a Map. Whitespace around text is trimmed. Invalid XML throws an error.

```pleno
import { XML } from "std:xml";

let doc = XML.parse(`
<book id="1">
  <title>Pleno Guide</title>
  <author>Team Pleno</author>
</book>
`);

print(doc.name);                     // book
print(doc.attributes.id);            // 1
print(doc.children[0].name);         // title
print(doc.children[0].children[0]);  // Pleno Guide
```

**Element Structure:**

| Property     | Type   | Description                    |
| ------------ | ------ | ------------------------------ |
| `name`       | String | Tag name                       |
| `attributes` | Map    | Attribute values, as strings   |
| `children`   | Array  | Child elements or text strings |

---
