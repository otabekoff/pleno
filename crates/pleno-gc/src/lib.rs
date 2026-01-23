use pleno_value::{MethodInfo, MethodVisibility, Range, Value, ValueKey};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::{Rc, Weak};

// A trait that all garbage collectible objects must implement
pub trait GcObject {
    fn mark(&self, gc: &mut MarkAndSweepGC);
    fn get_references(&self) -> Vec<Weak<RefCell<dyn GcObject>>>;
}

// Garbage collected version of Value
#[derive(Debug, Clone)]
pub enum GcValue {
    Null,
    Missing,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<String>),
    Range(Range),
    Class(GcRc<dyn GcObject>),
    Function(usize),
    BoundMethod {
        receiver: GcRc<dyn GcObject>,
        function: usize,
        owner: Rc<String>,
        visibility: MethodVisibility,
    },
    StaticMethod {
        function: usize,
        owner: Rc<String>,
        visibility: MethodVisibility,
    },
    Closure {
        function: usize,
        captures: Vec<GcValue>,
    },
    NativeFunction(fn(&[Value]) -> Result<Value, pleno_value::RuntimeError>),
    BoundNativeMethod {
        receiver: Box<GcValue>,
        function: fn(&[Value]) -> Result<Value, pleno_value::RuntimeError>,
    },
    Map(GcRc<HashMap<String, GcValue>>),
    Array(GcRc<Vec<GcValue>>),
    Set(GcRc<HashSet<ValueKey>>), // Assuming ValueKey is also GC-able
    Object(GcRc<dyn GcObject>),
    Partial {
        function: usize,
        args: Vec<GcValue>,
        placeholders: Vec<usize>,
    },
}

fn mark_gc_value(value: &GcValue, gc: &mut MarkAndSweepGC) {
    match value {
        GcValue::Class(class) | GcValue::Object(class) => {
            gc.mark_gc_object(class);
        }
        GcValue::BoundMethod { receiver, .. } => {
            gc.mark_gc_object(receiver);
        }
        GcValue::StaticMethod { .. } => {}
        GcValue::Closure { captures, .. } => {
            for capture in captures {
                mark_gc_value(capture, gc);
            }
        }
        GcValue::NativeFunction(_) => {}
        GcValue::BoundNativeMethod { receiver, .. } => {
            mark_gc_value(receiver, gc);
        }
        GcValue::Map(map) => {
            gc.mark_gc_object(&map.to_gc_object());
        }
        GcValue::Array(array) => {
            gc.mark_gc_object(&array.to_gc_object());
        }
        GcValue::Set(set) => {
            gc.mark_gc_object(&set.to_gc_object());
        }
        GcValue::Partial { args, .. } => {
            for arg in args {
                mark_gc_value(arg, gc);
            }
        }
        _ => {}
    }
}

impl GcObject for HashMap<String, GcValue> {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for value in self.values() {
            mark_gc_value(value, gc);
        }
    }

    fn get_references(&self) -> Vec<Weak<RefCell<dyn GcObject>>> {
        Vec::new()
    }
}

impl GcObject for Vec<GcValue> {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for value in self {
            mark_gc_value(value, gc);
        }
    }

    fn get_references(&self) -> Vec<Weak<RefCell<dyn GcObject>>> {
        Vec::new()
    }
}

impl GcObject for HashSet<ValueKey> {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {}

    fn get_references(&self) -> Vec<Weak<RefCell<dyn GcObject>>> {
        Vec::new()
    }
}

// Garbage collected version of Class
#[derive(Debug, Clone)]
pub struct GcClass {
    pub name: Rc<String>,
    pub methods: HashMap<String, MethodInfo>,
    pub static_methods: HashMap<String, MethodInfo>,
    pub constructor: Option<usize>,
    pub superclass: Option<GcRc<dyn GcObject>>,
    pub static_fields: HashMap<String, GcValue>,
    pub fields: HashMap<String, MethodVisibility>,
    pub static_field_visibility: HashMap<String, MethodVisibility>,
    pub module: Option<Rc<String>>,
}

// Garbage collected version of Object
#[derive(Debug, Clone)]
pub struct GcObjectWrapper {
    pub class: Option<GcRc<dyn GcObject>>,
    pub fields: HashMap<String, GcValue>,
}

// Wrapper for reference-counted objects that can be garbage collected
pub struct GcRc<T: ?Sized + GcObject> {
    inner: Rc<RefCell<T>>,
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

    pub fn get(&self) -> std::cell::Ref<'_, T> {
        self.inner.borrow()
    }

    pub fn get_mut(&self) -> std::cell::RefMut<'_, T> {
        self.inner.borrow_mut()
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

impl<T: ?Sized + GcObject> std::ops::Deref for GcRc<T> {
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
    allocation_threshold: usize,
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

        // Store a weak reference to the allocated object in the heap
        let weak_ref = gc_rc.downgrade();
        self.roots.push(weak_ref.inner.clone());
        self.heap.push(gc_rc.inner.clone());

        gc_rc
    }

    pub fn add_root(&mut self, obj: &GcRc<dyn GcObject>) {
        self.roots.push(Rc::downgrade(&obj.inner));
    }

    pub fn mark_gc_object(&mut self, obj: &GcRc<dyn GcObject>) {
        self.mark_object(&obj.inner);
    }

    pub fn collect(&mut self) {
        // Reset marked set
        self.marked.clear();

        // Mark phase: traverse from roots
        let roots_snapshot = self.roots.clone();
        for root_weak in roots_snapshot {
            if let Some(root_rc) = root_weak.upgrade() {
                self.mark_object(&root_rc);
            }
        }

        // Sweep phase: remove unmarked objects
        self.sweep();
    }

    fn mark_object(&mut self, obj: &Rc<RefCell<dyn GcObject>>) {
        let ptr = &*obj.borrow() as *const dyn GcObject;

        if self.marked.contains(&ptr) {
            return;
        }

        self.marked.insert(ptr);

        // Mark the object and its references
        obj.borrow().mark(self);
    }

    fn sweep(&mut self) {
        self.heap.retain(|obj| {
            let ptr = &*obj.borrow() as *const dyn GcObject;
            self.marked.contains(&ptr)
        });

        // Clean up dead weak references in roots
        self.roots.retain(|weak| weak.upgrade().is_some());
    }

    pub fn get_heap_size(&self) -> usize {
        self.heap.len()
    }

    pub fn get_marked_count(&self) -> usize {
        self.marked.len()
    }
}

// Implementation for GcClass
impl GcObject for GcClass {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        // Mark superclass if it exists
        if let Some(ref superclass) = self.superclass {
            let ptr = &*superclass.inner.borrow() as *const dyn GcObject;
            if !gc.marked.contains(&ptr) {
                gc.mark_object(&superclass.inner);
            }
        }

        // Mark static fields
        for field_value in self.static_fields.values() {
            mark_gc_value(field_value, gc);
        }
    }

    fn get_references(&self) -> Vec<Weak<RefCell<dyn GcObject>>> {
        let mut refs = Vec::new();

        if let Some(ref superclass) = self.superclass {
            refs.push(Rc::downgrade(&superclass.inner));
        }

        refs
    }
}

// Implementation for GcObjectWrapper
impl GcObject for GcObjectWrapper {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        // Mark class if it exists
        if let Some(ref class) = self.class {
            let ptr = &*class.inner.borrow() as *const dyn GcObject;
            if !gc.marked.contains(&ptr) {
                gc.mark_object(&class.inner);
            }
        }

        // Mark field values
        for field_value in self.fields.values() {
            mark_gc_value(field_value, gc);
        }
    }

    fn get_references(&self) -> Vec<Weak<RefCell<dyn GcObject>>> {
        let mut refs = Vec::new();

        if let Some(ref class) = self.class {
            refs.push(Rc::downgrade(&class.inner));
        }

        refs
    }
}

// GC Manager to coordinate garbage collection
pub struct GCManager {
    gc: MarkAndSweepGC,
    current_allocation: usize,
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

        // Trigger GC if we've exceeded the threshold
        if self.current_allocation >= self.gc.allocation_threshold {
            self.collect_garbage();
        }

        let gc_obj = self.gc.allocate(value);
        self.current_allocation = self.gc.get_heap_size();
        gc_obj
    }

    pub fn add_root(&mut self, obj: &GcRc<dyn GcObject>) {
        self.gc.add_root(obj);
    }

    pub fn collect_garbage(&mut self) {
        self.gc.collect();
        self.current_allocation = self.gc.get_heap_size();
    }

    pub fn get_stats(&self) -> GCStats {
        GCStats {
            heap_size: self.gc.get_heap_size(),
            marked_count: self.gc.get_marked_count(),
            threshold: self.gc.allocation_threshold,
            current_allocation: self.current_allocation,
        }
    }
}

#[derive(Debug)]
pub struct GCStats {
    pub heap_size: usize,
    pub marked_count: usize,
    pub threshold: usize,
    pub current_allocation: usize,
}

// A simple API function to initialize GC
pub fn initialize_gc(threshold: usize) -> GCManager {
    GCManager::new(threshold)
}
