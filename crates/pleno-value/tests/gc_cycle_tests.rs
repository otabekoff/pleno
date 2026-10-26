use pleno_value::{ArrayState, Value, gc::GCManager};

#[test]
fn test_cycle_collection() {
    let mut manager = GCManager::new(100); // Threshold 100

    // Create two arrays
    // A = []
    let array_a_state = ArrayState {
        elements: Vec::new(),
        is_immutable: false,
    };
    // Use the inner GC allocate directly or via manager?
    // Manager has .allocate().
    let array_a_rc = manager.allocate(array_a_state).unwrap();
    let array_a_value = Value::Array(array_a_rc.clone());

    // B = []
    let array_b_state = ArrayState {
        elements: Vec::new(),
        is_immutable: false,
    };
    let array_b_rc = manager.allocate(array_b_state).unwrap();
    let array_b_value = Value::Array(array_b_rc.clone());

    // Create cycle: A -> B, B -> A
    // We need to modify the arrays. GcRc gives us interior mutability via borrow_mut().
    // Value::Array(GcRc<ArrayState>)

    // Push B into A
    if let Value::Array(ref state) = array_a_value {
        state.borrow_mut().elements.push(array_b_value.clone());
    }

    // Push A into B
    if let Value::Array(ref state) = array_b_value {
        state.borrow_mut().elements.push(array_a_value.clone());
    }

    // Verify heap size (should be 2: array_a_state, array_b_state)
    assert_eq!(manager.gc.get_heap_size(), 2);

    // Keep weak refs to check later
    let weak_a = array_a_rc.downgrade();
    let weak_b = array_b_rc.downgrade();

    // Drop strong references
    drop(array_a_value);
    drop(array_b_value);
    drop(array_a_rc);
    drop(array_b_rc);

    // Collect garbage (no roots)
    manager.collect_garbage(&[]);

    // Verify heap size is 0
    assert_eq!(manager.gc.get_heap_size(), 0);

    // Verify weak refs are dead
    assert!(weak_a.upgrade().is_none());
    assert!(weak_b.upgrade().is_none());
}

#[test]
fn test_complex_cycle() {
    // A -> B -> C -> A
    let mut manager = GCManager::new(100);

    let mk_array = |mgr: &mut GCManager| {
        let state = ArrayState {
            elements: Vec::new(),
            is_immutable: false,
        };
        let rc = mgr.allocate(state).unwrap();
        (rc.clone(), Value::Array(rc))
    };

    let (rc_a, val_a) = mk_array(&mut manager);
    let (rc_b, val_b) = mk_array(&mut manager);
    let (rc_c, val_c) = mk_array(&mut manager);

    // A -> B
    if let Value::Array(ref s) = val_a {
        s.borrow_mut().elements.push(val_b.clone());
    }
    // B -> C
    if let Value::Array(ref s) = val_b {
        s.borrow_mut().elements.push(val_c.clone());
    }
    // C -> A
    if let Value::Array(ref s) = val_c {
        s.borrow_mut().elements.push(val_a.clone());
    }

    assert_eq!(manager.gc.get_heap_size(), 3);

    let weak_a = rc_a.downgrade();

    // Drop roots
    drop(val_a);
    drop(val_b);
    drop(val_c);
    drop(rc_a);
    drop(rc_b);
    drop(rc_c);

    manager.collect_garbage(&[]);

    assert_eq!(manager.gc.get_heap_size(), 0);
    assert!(weak_a.upgrade().is_none());
}

#[test]
fn releasing_the_heap_frees_cycles_and_keeps_held_values() {
    use pleno_value::{ClosureState, IteratorState};

    let mut manager = GCManager::new(100);
    let array = |manager: &mut GCManager| {
        manager
            .allocate(ArrayState {
                elements: Vec::new(),
                is_immutable: false,
            })
            .unwrap()
    };

    // A cycle through a closure that nothing outside the heap holds.
    let garbage = array(&mut manager);
    let closure = manager
        .allocate(ClosureState::new(0, vec![Value::Array(garbage.clone())]))
        .unwrap();
    garbage
        .borrow_mut()
        .elements
        .push(Value::Closure(closure.clone()));
    let garbage_weak = garbage.downgrade();
    drop((garbage, closure));

    // A cycle the embedder still holds, with an unreachable iterator over it.
    let kept = array(&mut manager);
    kept.borrow_mut().elements.push(Value::Array(kept.clone()));
    let iterator = manager
        .allocate(IteratorState::Array {
            array: kept.clone(),
            index: 0,
            is_entries: false,
        })
        .unwrap();
    drop(iterator);

    manager.release_all();

    assert!(garbage_weak.upgrade().is_none(), "the cycle is freed");
    assert_eq!(kept.borrow().elements, vec![Value::Array(kept.clone())]);
    assert_eq!(manager.gc.get_heap_size(), 0);
    kept.borrow_mut().elements.clear();
}

#[test]
fn releasing_the_heap_frees_nothing_while_an_object_is_borrowed() {
    let mut manager = GCManager::new(100);
    let array = manager
        .allocate(ArrayState {
            elements: Vec::new(),
            is_immutable: false,
        })
        .unwrap();
    array
        .borrow_mut()
        .elements
        .push(Value::Array(array.clone()));
    let weak = array.downgrade();
    {
        let _guard = array.borrow_mut();
        manager.release_all();
    }
    assert_eq!(array.borrow().elements.len(), 1, "left untouched");
    array.borrow_mut().elements.clear();
    drop(array);
    assert!(weak.upgrade().is_none());
}

#[test]
fn marking_a_long_chain_does_not_overflow_the_stack() {
    let mut manager = GCManager::new(usize::MAX);
    let new_array = |manager: &mut GCManager, elements: Vec<Value>| {
        manager
            .allocate(ArrayState {
                elements,
                is_immutable: false,
            })
            .unwrap()
    };
    let mut head = new_array(&mut manager, Vec::new());
    for _ in 0..200_000 {
        head = new_array(&mut manager, vec![Value::Array(head)]);
    }
    let root = Value::Array(head.clone());
    manager.collect_garbage(&[&root]);
    assert_eq!(manager.gc.get_heap_size(), 200_001);
    drop(root);

    // Take the chain apart link by link: dropping it whole would recurse.
    let mut link = Some(head);
    while let Some(array) = link {
        link = match array.borrow_mut().elements.pop() {
            Some(Value::Array(next)) => Some(next),
            _ => None,
        };
    }
}
