# std:xml

XML parsing.

## Import

```pleno
import { XML } from "std:xml";
```

## Functions

### `parse(xml: String): Map`

Parses an XML string into a tree structure.

```pleno
let doc = XML.parse(`
<book id="1">
  <title>Pleno Guide</title>
  <author>Team Pleno</author>
</book>
`);

print(doc.name);                     // "book"
print(doc.attributes.id);            // "1"
print(doc.children[0].children[0]);  // "Pleno Guide"
```

**Element Structure:**

| Property     | Type   | Description                    |
| ------------ | ------ | ------------------------------ |
| `name`       | String | Tag name                       |
| `attributes` | Map    | Attribute key-value pairs      |
| `children`   | Array  | Child elements or text strings |

---
