//! Hosts that run many programs in one process (the language server, the
//! REPL, the playground) must not grow: once a VM is dropped, the memory its
//! run allocated is free again (issue #60).

use pleno_vm::VM;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

/// Counts the bytes currently allocated by the whole test binary.
struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(
            new_size as isize - layout.size() as isize,
            Ordering::Relaxed,
        );
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn run(source: &str) {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let chunk = pleno_compiler::Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).unwrap();
}

const PROGRAM: &str = r#"
    class Node {
        constructor(name) { this.name = name; this.next = null; this.items = []; }
        fn label() { return "node " + this.name; }
    }
    let first = new Node("a");
    let second = new Node("b");
    first.next = second;
    second.next = first;
    first.items.push(first);
    first.items.push(second.label);
    let me = first;
    first.callback = fn() { return me.label(); };
    let table = {"self": null, "list": [1, 2, 3]};
    table["self"] = table;
    fn make() {
        let slot = {"f": null};
        slot["f"] = fn(n) {
            if (n == 0) { return 0; }
            return slot["f"](n - 1);
        };
        return slot["f"];
    }
    let countdown = make();
    print(first.callback() + " " + countdown(5));
"#;

#[test]
fn running_programs_repeatedly_does_not_grow_memory() {
    // The first runs fill caches that live for the whole thread.
    for _ in 0..3 {
        run(PROGRAM);
    }
    let before = LIVE.load(Ordering::Relaxed);
    for _ in 0..50 {
        run(PROGRAM);
    }
    let growth = LIVE.load(Ordering::Relaxed) - before;
    // Leaked cycles cost several kilobytes per run.
    assert!(
        growth < 4096,
        "50 runs left {growth} more bytes allocated than before"
    );
    // With no VM left on the thread, no NaN-boxed string, big integer or
    // range can be in use.
    assert_eq!(
        pleno_value::nan_value::boxed_value_counts(),
        (0, 0, 0),
        "big integers, strings and ranges still in their tables"
    );
}
