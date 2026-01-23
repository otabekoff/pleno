use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;
use std::rc::{Rc, Weak};

// A trait that all garbage collectible objects must implement
pub trait GcObject {
    fn mark(&self, gc: &mut MarkAndSweepGC);
    fn clear_refs(&mut self) {} // Default implementation does nothing
}

// Wrapper for reference-counted objects that can be garbage collected
pub struct GcRc<T: ?Sized + GcObject> {
    pub(crate) inner: Rc<RefCell<T>>,
}

impl<T: GcObject + 'static> GcRc<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(RefCell::new(value)),
        }
    }

    pub fn to_gc_object(&self) -> GcRc<dyn GcObject> {
        let inner: Rc<RefCell<dyn GcObject>> = self.inner.clone();
        GcRc { inner }
    }
}

impl<T: ?Sized + GcObject> GcRc<T> {
    pub fn downgrade(&self) -> GcWeak<T> {
        GcWeak {
            inner: Rc::downgrade(&self.inner),
        }
    }

    pub fn borrow(&self) -> Ref<'_, T> {
        self.inner.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }

    pub fn as_ptr(&self) -> *const T {
        self.inner.as_ptr()
    }
}

impl<T: ?Sized + GcObject> Clone for GcRc<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: ?Sized + GcObject> fmt::Debug for GcRc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcRc")
            .field("ptr", &Rc::as_ptr(&self.inner))
            .finish()
    }
}

impl<T: ?Sized + GcObject> PartialEq for GcRc<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: ?Sized + GcObject> Eq for GcRc<T> {}

impl<T: ?Sized + GcObject> Deref for GcRc<T> {
    type Target = Rc<RefCell<T>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// Weak reference for breaking cycles
#[derive(Debug)]
pub struct GcWeak<T: ?Sized + GcObject> {
    inner: Weak<RefCell<T>>,
}

impl<T: ?Sized + GcObject> GcWeak<T> {
    pub fn upgrade(&self) -> Option<GcRc<T>> {
        Weak::upgrade(&self.inner).map(|rc| GcRc { inner: rc })
    }
}

// Mark and Sweep Garbage Collector
pub struct MarkAndSweepGC {
    roots: Vec<Weak<RefCell<dyn GcObject>>>,
    heap: Vec<Rc<RefCell<dyn GcObject>>>,
    marked: HashSet<*const dyn GcObject>,
    pub allocation_threshold: usize,
}

impl MarkAndSweepGC {
    pub fn new(allocation_threshold: usize) -> Self {
        Self {
            roots: Vec::new(),
            heap: Vec::new(),
            marked: HashSet::new(),
            allocation_threshold,
        }
    }

    pub fn allocate<T>(&mut self, value: T) -> GcRc<T>
    where
        T: GcObject + 'static,
    {
        let gc_rc = GcRc::new(value);

        // Store a reference to the allocated object in the heap
        self.heap.push(gc_rc.inner.clone());

        gc_rc
    }

    pub fn add_root(&mut self, obj: &GcRc<dyn GcObject>) {
        self.roots.push(Rc::downgrade(&obj.inner));
    }

    pub fn collect(&mut self, manual_roots: &[&dyn GcObject]) {
        // Reset marked set
        self.marked.clear();

        // Mark phase: traverse from permanent roots
        let roots_snapshot = self.roots.clone();
        for root_weak in roots_snapshot {
            if let Some(root_rc) = root_weak.upgrade() {
                self.mark_object(&GcRc { inner: root_rc });
            }
        }

        // Mark phase: traverse from manual roots (stack, etc.)
        for root in manual_roots {
            root.mark(self);
        }

        // Sweep phase: remove unmarked objects
        self.sweep();
    }

    pub fn mark_object(&mut self, obj: &GcRc<dyn GcObject>) {
        let ptr = obj.inner.as_ptr() as *const dyn GcObject;

        if self.marked.contains(&ptr) {
            return;
        }

        self.marked.insert(ptr);

        // Mark the object and its references
        obj.inner.borrow().mark(self);
    }

    fn sweep(&mut self) {
        // Break cycles for unmarked objects
        for obj in &self.heap {
            let ptr = obj.as_ptr() as *const dyn GcObject;
            if !self.marked.contains(&ptr) {
                obj.borrow_mut().clear_refs();
            }
        }

        self.heap.retain(|obj| {
            let ptr = obj.as_ptr() as *const dyn GcObject;
            self.marked.contains(&ptr)
        });

        // Clean up dead weak references in roots
        self.roots.retain(|weak| weak.upgrade().is_some());
    }

    pub fn get_heap_size(&self) -> usize {
        self.heap.len()
    }
}

// GC Manager to coordinate garbage collection
pub struct GCManager {
    pub gc: MarkAndSweepGC,
    pub current_allocation: usize,
}

impl GCManager {
    pub fn new(threshold: usize) -> Self {
        Self {
            gc: MarkAndSweepGC::new(threshold),
            current_allocation: 0,
        }
    }

    pub fn allocate<T>(&mut self, value: T) -> GcRc<T>
    where
        T: GcObject + 'static,
    {
        self.current_allocation += 1;
        self.gc.allocate(value)
    }

    pub fn maybe_collect(&mut self, roots: &[&dyn GcObject]) {
        if self.current_allocation >= self.gc.allocation_threshold {
            self.collect_garbage(roots);
        }
    }

    pub fn collect_garbage(&mut self, roots: &[&dyn GcObject]) {
        self.gc.collect(roots);
        self.current_allocation = self.gc.get_heap_size();
    }
}
