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
