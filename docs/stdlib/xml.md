# std:xml

> **Stability:** stable in 1.0.

XML parsing.

## Import

```pleno
import { XML } from "std:xml";
```

`parse` can also be imported directly.

## Functions

### `parse(xml: String): Map`

Parses an XML string and returns its root element as a Map. Whitespace around text is trimmed. Malformed XML throws an error, as `JSON.parse` does for invalid JSON: a mismatched closing tag, an unquoted attribute, an element that is never closed (`XML parse error: element <a> is not closed`) or input without a root element, such as an empty string or only a comment (`XML parse error: no root element`).

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

try {
    XML.parse("<a>");
} catch (e) {
    print(e.message);  // XML parse error: element <a> is not closed
}
```

**Element Structure:**

| Property     | Type   | Description                    |
| ------------ | ------ | ------------------------------ |
| `name`       | String | Tag name                       |
| `attributes` | Map    | Attribute values, as strings   |
| `children`   | Array  | Child elements or text strings |

---
