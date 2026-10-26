//! Dropping a VM frees every value it allocated, including values that only
//! refer to each other (issue #60), while values the caller still holds stay
//! intact.

use pleno_value::Value;
use pleno_vm::VM;
use std::rc::Rc;

/// Runs `source` and returns the VM with the program's result.
fn run(source: &str) -> (VM, Value) {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let chunk = pleno_compiler::Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    let result = vm.execute(chunk).unwrap();
    (vm, result)
}

/// A check that the value behind `value` has been freed.
fn freed(value: &Value) -> Box<dyn Fn() -> bool> {
    macro_rules! weak {
        ($rc:expr) => {{
            let weak = Rc::downgrade(&**$rc);
            Box::new(move || weak.upgrade().is_none())
        }};
    }
    match value {
        Value::Object(o) => weak!(o),
        Value::Closure(c) => weak!(c),
        Value::Array(a) => weak!(a),
        Value::Map(m) => weak!(m),
        Value::Class(c) => weak!(c),
        Value::BoundMethod(m) => weak!(m),
        other => panic!("no weak reference for {other:?}"),
    }
}

/// Runs `source`, which returns an array of values, drops the VM and the
/// result, and returns the indexes of the values still allocated.
fn leaked_after_drop(source: &str) -> Vec<usize> {
    let (vm, result) = run(source);
    let Value::Array(values) = &result else {
        panic!("the program must return an array, not {result:?}");
    };
    let checks: Vec<_> = values.borrow().elements.iter().map(freed).collect();
    drop(result);
    drop(vm);
    (0..checks.len()).filter(|&i| !checks[i]()).collect()
}

#[test]
fn objects_pointing_at_themselves_are_freed() {
    let leaked = leaked_after_drop(
        r#"
        class Node {
            constructor() { this.next = null; }
        }
        let node = new Node();
        node.next = node;
        let a = new Node();
        let b = new Node();
        a.next = b;
        b.next = a;
        return [node, a, b, Node];
        "#,
    );
    assert!(leaked.is_empty(), "still allocated: {leaked:?}");
}

#[test]
fn collections_containing_themselves_are_freed() {
    let leaked = leaked_after_drop(
        r#"
        let list = [1, 2];
        list.push(list);
        let map = {"x": 1};
        map["self"] = map;
        return [list, map];
        "#,
    );
    assert!(leaked.is_empty(), "still allocated: {leaked:?}");
}

#[test]
fn closure_reaching_itself_is_freed() {
    let leaked = leaked_after_drop(
        r#"
        fn make() {
            let slot = {"f": null};
            slot["f"] = fn(n) {
                if (n == 0) { return 0; }
                return slot["f"](n - 1);
            };
            return slot["f"];
        }
        let countdown = make();
        countdown(3);
        return [countdown];
        "#,
    );
    assert!(leaked.is_empty(), "still allocated: {leaked:?}");
}

#[test]
fn method_closure_capturing_its_instance_is_freed() {
    let leaked = leaked_after_drop(
        r#"
        class Counter {
            constructor() {
                this.count = 0;
                let me = this;
                this.bump = fn() { me.count = me.count + 1; return me.count; };
            }
            fn get() { return this.count; }
        }
        let counter = new Counter();
        counter.bump();
        let getter = counter.get;
        getter();
        return [counter, Counter, getter];
        "#,
    );
    assert!(leaked.is_empty(), "still allocated: {leaked:?}");
}

#[test]
fn values_the_caller_holds_survive_the_vm() {
    let (vm, result) = run(r#"
        class Node {
            constructor(name) { this.name = name; this.next = null; }
        }
        let node = new Node("kept");
        node.next = node;
        let garbage = new Node("dropped");
        garbage.next = garbage;
        return [[node, "text"], garbage];
        "#);
    let Value::Array(pair) = &result else {
        panic!("expected an array, got {result:?}");
    };
    let kept = pair.borrow().elements[0].clone();
    let garbage_freed = freed(&pair.borrow().elements[1]);
    drop(result);
    drop(vm);

    assert!(garbage_freed(), "the cycle nothing holds is freed");

    // The held array and everything it reaches are untouched.
    let Value::Array(list) = &kept else {
        panic!("expected an array, got {kept:?}");
    };
    let elements = list.borrow().elements.clone();
    assert_eq!(elements.len(), 2);
    let Value::Object(node) = &elements[0] else {
        panic!("expected an object, got {:?}", elements[0]);
    };
    let fields = node.borrow().fields.clone();
    assert_eq!(
        fields.get("name").map(|v| v.to_string()),
        Some("kept".into())
    );
    assert_eq!(fields.get("next"), Some(&elements[0]));
    assert!(node.borrow().class.is_some(), "the class stays too");

    // Break the cycle so the test itself does not leak.
    node.borrow_mut().fields.clear();
}

#[test]
fn collection_keeps_what_an_unreachable_iterator_iterated() {
    // A collection emptied the array or map behind an iterator that had
    // become garbage, though the program still used them.
    let (_vm, result) = run(r#"
        import { collect } from "std:gc";
        let list = [1, 2, 3];
        let map = {"x": 1};
        fn walk() {
            let it = list.iterator();
            it.next();
            for (entry in map.entries()) { }
        }
        walk();
        collect();
        return [list.length, map.keys().length];
        "#);
    assert_eq!(result.to_string(), "[3, 1]");
}

#[test]
fn cycles_through_error_static_fields_are_freed() {
    // `Error` is created with the VM; a cycle stored on it was kept alive by
    // it before it lived on the heap.
    let leaked = leaked_after_drop(
        r#"
        let box = {"self": None};
        box["self"] = box;
        Error.cache = box;
        [box, Error];
        "#,
    );
    assert!(leaked.is_empty(), "still allocated: {leaked:?}");
}

#[test]
fn the_error_class_survives_collections() {
    // A collection while the program runs must keep `Error` (a root even
    // when no global names it) and everything on it.
    let (_vm, result) = run(r#"
        Error.marker = "kept";
        let Error2 = Error;
        let mut junk = [];
        for (i in 0..20000) { junk = [junk, {"i": i}]; }
        try { 1 / 0; } catch (e) { e.name + " " + Error2.marker + " " + (e is Error); }
        "#);
    assert_eq!(result.to_string(), "Error kept true");
}
