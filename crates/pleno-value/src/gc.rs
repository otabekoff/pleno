use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Deref;
use std::rc::{Rc, Weak};

// A trait that all garbage collectible objects must implement
pub trait GcObject {
    fn mark(&self, gc: &mut MarkAndSweepGC);
    fn clear_refs(&mut self) {} // Default implementation does nothing
    /// Drops every reference to other values that `mark` visits, so that
    /// objects freed together (see `MarkAndSweepGC::release_unreferenced`)
    /// cannot keep each other alive. Collections use `clear_refs` instead;
    /// this runs only on objects that nothing can reach any more.
    fn release_refs(&mut self) {
        self.clear_refs();
    }
    fn byte_size(&self) -> usize {
        std::mem::size_of_val(self)
    }
}

// Wrapper for reference-counted objects that can be garbage collected
#[derive(Serialize, Deserialize)]
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

    pub fn as_refcell_ptr(&self) -> *const RefCell<T> {
        Rc::as_ptr(&self.inner)
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
    marked: HashSet<*const ()>,
    /// While `release_unreferenced` counts the references between heap
    /// objects, `mark_object` adds to these counts instead of marking.
    incoming: Option<HashMap<*const (), usize>>,
    /// Marked objects whose references are still to be marked.
    pending: Vec<GcRc<dyn GcObject>>,
    marking: bool,
    pub allocation_threshold: usize,
}

impl MarkAndSweepGC {
    pub fn new(allocation_threshold: usize) -> Self {
        Self {
            roots: Vec::new(),
            heap: Vec::new(),
            marked: HashSet::new(),
            incoming: None,
            pending: Vec::new(),
            marking: false,
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

    /// Forgets the marks of the last collection, and any marking a panic
    /// interrupted.
    fn reset_marks(&mut self) {
        self.marked.clear();
        self.pending.clear();
        self.marking = false;
    }

    pub fn collect(&mut self, manual_roots: &[&dyn GcObject]) {
        self.reset_marks();

        let roots_snapshot = self.roots.clone();
        for root_weak in roots_snapshot {
            if let Some(root_rc) = root_weak.upgrade() {
                self.mark_object(&GcRc { inner: root_rc });
            }
        }

        for root in manual_roots {
            root.mark(self);
        }

        self.sweep();
    }

    pub fn collect_with<F>(&mut self, marker: F)
    where
        F: FnOnce(&mut MarkAndSweepGC),
    {
        self.reset_marks();

        let roots_snapshot = self.roots.clone();
        for root_weak in roots_snapshot {
            if let Some(root_rc) = root_weak.upgrade() {
                self.mark_object(&GcRc { inner: root_rc });
            }
        }

        marker(self);

        self.sweep();
    }

    pub fn mark_object(&mut self, obj: &GcRc<dyn GcObject>) {
        let ptr = Rc::as_ptr(&obj.inner) as *const ();

        if let Some(incoming) = &mut self.incoming {
            *incoming.entry(ptr).or_insert(0) += 1;
            return;
        }

        if !self.marked.insert(ptr) {
            return;
        }

        // Objects found while marking wait in `pending` instead of being
        // marked recursively, so a long linked list cannot overflow the stack.
        if self.marking {
            self.pending.push(obj.clone());
            return;
        }
        self.marking = true;
        obj.inner.borrow().mark(self);
        while let Some(next) = self.pending.pop() {
            next.inner.borrow().mark(self);
        }
        self.marking = false;
    }

    fn sweep(&mut self) {
        for obj in &self.heap {
            let ptr = Rc::as_ptr(obj) as *const ();
            if !self.marked.contains(&ptr) {
                obj.borrow_mut().clear_refs();
            }
        }

        self.heap.retain(|obj| {
            let ptr = Rc::as_ptr(obj) as *const ();
            self.marked.contains(&ptr)
        });

        self.roots.retain(|weak| weak.upgrade().is_some());
    }

    /// Empties the heap for good, as when its VM goes away, and frees every
    /// object that nothing outside the heap refers to, including objects that
    /// only refer to each other.
    ///
    /// An object is still in use when it has more strong references than the
    /// heap's own and those from other heap objects: a `Value` the embedder
    /// kept (the result of `execute`, say), a global of another VM, a native
    /// structure. Those objects, and everything they reach, are left intact
    /// and are freed by reference counting once their last holder lets go;
    /// a cycle among them is not collected after that, as no collector is
    /// left. Every other object drops its references (`release_refs`), which
    /// breaks the cycles, and is freed.
    ///
    /// A reference that some `mark` does not visit can only keep its target
    /// (and what the target reaches) alive, never free it early. Returns the
    /// number of objects released.
    pub fn release_unreferenced(&mut self) -> usize {
        let heap = std::mem::take(&mut self.heap);
        self.roots.clear();
        self.reset_marks();

        // An object borrowed elsewhere cannot be inspected: free nothing then.
        if heap.iter().any(|obj| obj.try_borrow_mut().is_err()) {
            return 0;
        }

        // References between heap objects.
        self.incoming = Some(HashMap::with_capacity(heap.len()));
        for obj in &heap {
            obj.borrow().mark(self);
        }
        let incoming = self.incoming.take().unwrap_or_default();

        // Objects held from outside the heap, and what they reach, stay.
        for obj in &heap {
            let ptr = Rc::as_ptr(obj) as *const ();
            let internal = incoming.get(&ptr).copied().unwrap_or(0);
            if Rc::strong_count(obj) > internal + 1 {
                self.mark_object(&GcRc { inner: obj.clone() });
            }
        }

        let mut released = 0;
        for obj in &heap {
            let ptr = Rc::as_ptr(obj) as *const ();
            if !self.marked.contains(&ptr) {
                obj.borrow_mut().release_refs();
                released += 1;
            }
        }
        self.marked.clear();
        drop(heap);
        released
    }

    pub fn get_heap_size(&self) -> usize {
        self.heap.len()
    }
}

pub struct GCManager {
    pub gc: MarkAndSweepGC,
    pub current_allocation: usize, // number of objects
    pub max_allocation: usize,     // max number of objects (0 = unlimited)
    pub allocated_bytes: usize,    // total bytes currently allocated
    pub peak_bytes: usize,         // peak bytes allocated
    pub max_bytes: usize,          // max bytes allowed (0 = unlimited)
    pub grace_bytes: usize,        // extra bytes allowed during emergency (e.g. error handling)

    // Statistics tracking
    pub total_collections: u64,
    /// Whether collections also free unused slots of NaN-boxed big integers,
    /// strings and ranges. Only the VM that owns the thread's registers may do
    /// this; sandboxed VMs share them.
    pub sweeps_boxed_values: bool,
    pub total_freed_bytes: usize,
    pub total_freed_objects: usize,
    pub last_collection_freed_bytes: usize,
}

/// Dropping the heap (when its VM goes away) frees every object it allocated,
/// including objects that only refer to each other, which reference counting
/// alone never frees.
///
/// Objects still held from outside the heap, such as a `Value` returned by a
/// run and kept by the embedder, stay intact together with everything they
/// reach: they keep working and are freed by reference counting once their
/// holders drop them. A cycle among those is then no longer collected.
impl Drop for GCManager {
    fn drop(&mut self) {
        self.release_all();
    }
}

impl GCManager {
    pub fn new(threshold: usize) -> Self {
        Self {
            gc: MarkAndSweepGC::new(threshold),
            current_allocation: 0,
            max_allocation: 0,
            allocated_bytes: 0,
            peak_bytes: 0,
            max_bytes: 0,
            grace_bytes: 0,
            total_collections: 0,
            sweeps_boxed_values: true,
            total_freed_bytes: 0,
            total_freed_objects: 0,
            last_collection_freed_bytes: 0,
        }
    }

    pub fn track_allocation(&mut self, bytes: usize) -> Result<(), crate::RuntimeError> {
        if self.max_bytes > 0 && self.allocated_bytes + bytes > self.max_bytes + self.grace_bytes {
            // Fatal: `catch` must not let a program run on past its limit.
            return Err(crate::RuntimeError::fatal(
                "SandboxError: memory limit exceeded",
            ));
        }
        if self.max_allocation > 0 && self.current_allocation + 1 > self.max_allocation {
            return Err(crate::RuntimeError::fatal(
                "SandboxError: object allocation limit exceeded",
            ));
        }
        self.allocated_bytes += bytes;
        if self.allocated_bytes > self.peak_bytes {
            self.peak_bytes = self.allocated_bytes;
        }
        self.current_allocation += 1;
        Ok(())
    }

    pub fn allocate<T>(&mut self, value: T) -> Result<GcRc<T>, crate::RuntimeError>
    where
        T: GcObject + 'static,
    {
        self.track_allocation(std::mem::size_of::<T>())?;
        Ok(self.gc.allocate(value))
    }

    /// Frees the heap for good (see `MarkAndSweepGC::release_unreferenced`);
    /// dropping the manager does this.
    pub fn release_all(&mut self) {
        let released = self.gc.release_unreferenced();
        self.total_freed_objects += released;
        self.current_allocation = 0;
        self.allocated_bytes = 0;
    }

    pub fn maybe_collect(&mut self, roots: &[&dyn GcObject]) {
        if self.current_allocation >= self.gc.allocation_threshold {
            self.collect_garbage(roots);
        }
    }

    pub fn collect_garbage(&mut self, roots: &[&dyn GcObject]) {
        let before_bytes = self.allocated_bytes;
        let before_objects = self.current_allocation;

        self.gc.collect(roots);
        self.current_allocation = self.gc.get_heap_size();

        // Sum up the actual sizes of survivors
        let mut total_bytes = 0;
        for obj in &self.gc.heap {
            total_bytes += obj.borrow().byte_size();
        }

        self.allocated_bytes = total_bytes;

        // Update statistics
        let freed_bytes = before_bytes.saturating_sub(total_bytes);
        let freed_objects = before_objects.saturating_sub(self.current_allocation);

        self.total_collections += 1;
        if self.sweeps_boxed_values {
            crate::nan_value::sweep_boxed_values();
        }
        self.total_freed_bytes += freed_bytes;
        self.total_freed_objects += freed_objects;
        self.last_collection_freed_bytes = freed_bytes;
    }

    pub fn report(&self) {
        println!("--- Pleno GC Report ---");
        println!("Collections:       {}", self.total_collections);
        println!("Heap Size:         {} objects", self.current_allocation);
        println!("Allocated:         {} bytes", self.allocated_bytes);
        println!("Peak Usage:        {} bytes", self.peak_bytes);
        println!(
            "Total Freed:       {} bytes ({} objects)",
            self.total_freed_bytes, self.total_freed_objects
        );
        println!(
            "Last Freed:        {} bytes",
            self.last_collection_freed_bytes
        );
        println!("-----------------------");
    }

    pub fn stats(&self) -> IndexMap<String, crate::Value> {
        let mut stats = IndexMap::new();
        stats.insert(
            "collections".to_string(),
            crate::Value::Int(self.total_collections as i64),
        );
        stats.insert(
            "current_objects".to_string(),
            crate::Value::Int(self.current_allocation as i64),
        );
        stats.insert(
            "current_bytes".to_string(),
            crate::Value::Int(self.allocated_bytes as i64),
        );
        stats.insert(
            "peak_bytes".to_string(),
            crate::Value::Int(self.peak_bytes as i64),
        );
        stats.insert(
            "total_freed_bytes".to_string(),
            crate::Value::Int(self.total_freed_bytes as i64),
        );
        stats.insert(
            "total_freed_objects".to_string(),
            crate::Value::Int(self.total_freed_objects as i64),
        );
        stats.insert(
            "last_freed_bytes".to_string(),
            crate::Value::Int(self.last_collection_freed_bytes as i64),
        );
        stats
    }

    pub fn collect_garbage_with<F>(&mut self, marker: F)
    where
        F: FnOnce(&mut MarkAndSweepGC),
    {
        let before_bytes = self.allocated_bytes;
        let before_objects = self.current_allocation;

        self.gc.collect_with(marker);
        self.current_allocation = self.gc.get_heap_size();

        // Sum up the actual sizes of survivors
        let mut total_bytes = 0;
        for obj in &self.gc.heap {
            total_bytes += obj.borrow().byte_size();
        }

        self.allocated_bytes = total_bytes;

        // Update statistics
        let freed_bytes = before_bytes.saturating_sub(total_bytes);
        let freed_objects = before_objects.saturating_sub(self.current_allocation);

        self.total_collections += 1;
        if self.sweeps_boxed_values {
            crate::nan_value::sweep_boxed_values();
        }
        self.total_freed_bytes += freed_bytes;
        self.total_freed_objects += freed_objects;
        self.last_collection_freed_bytes = freed_bytes;
    }
}
