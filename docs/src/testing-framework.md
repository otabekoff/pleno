## 15. Testing Framework

```pleno
import { test, assert, describe, beforeEach, afterEach } from "std:test";

// Test suite
describe("Calculator", fn() {
    let calc: Calculator;
    
    beforeEach(fn() {
        calc = Calculator();
    });
    
    afterEach(fn() {
        calc = null;
    });
    
    test("should add numbers correctly", fn() {
        let result = calc.add(2, 3);
        assert.equal(result, 5);
    });
    
    test("should handle negative numbers", fn() {
        let result = calc.add(-5, 3);
        assert.equal(result, -2);
    });
    
    test("should throw on division by zero", fn() {
        assert.throws(fn() => calc.divide(10, 0));
    });
});

// Assertions
assert.equal(actual, expected);
assert.notEqual(actual, expected);
assert.true(value);
assert.false(value);
assert.null(value);
assert.notNull(value);
assert.throws(fn);
assert.contains(collection, item);
assert.lengthOf(collection, length);
```