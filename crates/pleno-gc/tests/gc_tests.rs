use pleno_gc::{GcObject, GcRc, initialize_gc};

// A simple test object that implements GcObject
#[derive(Debug, Clone)]
struct TestObject {
    value: i32,
    children: Vec<GcRc<TestObject>>,
}

impl TestObject {
    fn new(value: i32) -> Self {
        Self {
            value,
            children: Vec::new(),
        }
    }

    fn add_child(&mut self, child: GcRc<TestObject>) {
        self.children.push(child);
    }
}

impl GcObject for TestObject {
    fn mark(&self, gc: &mut pleno_gc::MarkAndSweepGC) {
        // Mark this object and its children
        for child in &self.children {
            gc.mark_gc_object(&child.to_gc_object());
        }
    }

    fn get_references(&self) -> Vec<std::rc::Weak<std::cell::RefCell<dyn GcObject>>> {
        Vec::new()
    }
}

#[test]
fn test_basic_allocation() {
    let mut gc_manager = initialize_gc(10);

    let obj = TestObject::new(42);
    let gc_obj = gc_manager.allocate(obj);

    assert_eq!(gc_obj.get().value, 42);
    assert_eq!(gc_manager.get_stats().heap_size, 1);
}

#[test]
fn test_multiple_allocations() {
    let mut gc_manager = initialize_gc(10);

    let obj1 = gc_manager.allocate(TestObject::new(1));
    let obj2 = gc_manager.allocate(TestObject::new(2));
    let obj3 = gc_manager.allocate(TestObject::new(3));

    assert_eq!(obj1.get().value, 1);
    assert_eq!(obj2.get().value, 2);
    assert_eq!(obj3.get().value, 3);

    assert_eq!(gc_manager.get_stats().heap_size, 3);
}

#[test]
fn test_object_references() {
    let mut gc_manager = initialize_gc(10);

    let child = gc_manager.allocate(TestObject::new(100));
    let mut parent = TestObject::new(200);
    parent.add_child(child);
    let parent = gc_manager.allocate(parent);

    assert_eq!(parent.get().children.len(), 1);
    assert_eq!(parent.get().children[0].get().value, 100);
}

#[test]
fn test_garbage_collection() {
    let mut gc_manager = initialize_gc(2); // Low threshold to trigger GC

    // Allocate some objects
    let obj1 = gc_manager.allocate(TestObject::new(1));
    let obj2 = gc_manager.allocate(TestObject::new(2));

    // Add obj2 as a child to obj1 to keep it alive
    {
        let mut obj1_mut = obj1.get_mut();
        obj1_mut.add_child(obj2.clone());
    }

    // Allocate more objects to trigger GC
    let obj3 = gc_manager.allocate(TestObject::new(3));

    // At this point, GC may have been triggered
    let stats = gc_manager.get_stats();
    assert!(stats.heap_size >= 3); // Should have at least 3 objects

    // obj2 should still be accessible through obj1's children
    assert_eq!(obj1.get().children.len(), 1);
    assert_eq!(obj1.get().children[0].get().value, 2);

    // obj3 should be accessible
    assert_eq!(obj3.get().value, 3);
}

#[test]
fn test_add_root_functionality() {
    let mut gc_manager = initialize_gc(10);

    let obj = gc_manager.allocate(TestObject::new(42));

    // Add as root to ensure it's kept alive
    let root_obj = obj.to_gc_object();
    gc_manager.add_root(&root_obj);

    // Force collection
    gc_manager.collect_garbage();

    // Object should still be accessible
    assert_eq!(obj.get().value, 42);

    let stats = gc_manager.get_stats();
    assert!(stats.heap_size >= 1);
}

#[test]
fn test_garbage_collection_with_cycles() {
    let mut gc_manager = initialize_gc(10);

    // Create a cycle: obj1 -> obj2 -> obj1
    let obj1 = gc_manager.allocate(TestObject::new(1));
    let obj2 = gc_manager.allocate(TestObject::new(2));

    // Make obj1 hold a reference to obj2
    {
        let mut obj1_mut = obj1.get_mut();
        obj1_mut.add_child(obj2.clone());
    }

    // Make obj2 hold a reference to obj1
    {
        let mut obj2_mut = obj2.get_mut();
        obj2_mut.add_child(obj1.clone());
    }

    // Both objects should be accessible
    assert_eq!(obj1.get().value, 1);
    assert_eq!(obj2.get().value, 2);
    assert_eq!(obj1.get().children.len(), 1);
    assert_eq!(obj2.get().children.len(), 1);

    // Force collection
    gc_manager.collect_garbage();

    // Objects should still be accessible if they're reachable from roots
    let stats = gc_manager.get_stats();
    // The cycle should be detected and handled appropriately
    assert!(stats.heap_size >= 2);
}

#[test]
fn test_garbage_collection_statistics() {
    let mut gc_manager = initialize_gc(5); // Small threshold

    // Initially empty
    let initial_stats = gc_manager.get_stats();
    assert_eq!(initial_stats.heap_size, 0);
    assert_eq!(initial_stats.marked_count, 0);

    // Add some objects
    let obj1 = gc_manager.allocate(TestObject::new(10));
    let _obj2 = gc_manager.allocate(TestObject::new(20));

    let mid_stats = gc_manager.get_stats();
    assert_eq!(mid_stats.heap_size, 2);

    // Add root to keep obj1 alive
    let obj1_root = obj1.to_gc_object();
    gc_manager.add_root(&obj1_root);

    // Force collection
    gc_manager.collect_garbage();

    let final_stats = gc_manager.get_stats();
    // After collection, we should still have at least obj1 if it's a root
    assert!(final_stats.heap_size >= 1);
    assert!(final_stats.marked_count >= 1);
}

#[test]
fn test_gc_threshold_triggers_collection() {
    let mut gc_manager = initialize_gc(3); // Very low threshold

    // Add objects to trigger automatic GC
    let obj1 = gc_manager.allocate(TestObject::new(1));
    let obj2 = gc_manager.allocate(TestObject::new(2));
    let obj3 = gc_manager.allocate(TestObject::new(3));

    // At this point, GC should have been triggered automatically
    // due to exceeding the threshold
    let stats = gc_manager.get_stats();
    assert!(stats.current_allocation >= 3);

    // Objects should still be accessible if they're still referenced
    assert_eq!(obj1.get().value, 1);
    assert_eq!(obj2.get().value, 2);
    assert_eq!(obj3.get().value, 3);
}

#[test]
fn test_clone_functionality() {
    let mut gc_manager = initialize_gc(10);

    let obj = gc_manager.allocate(TestObject::new(999));
    let cloned_obj = obj.clone();

    assert_eq!(obj.get().value, cloned_obj.get().value);
    assert_eq!(obj.get().value, 999);
}
