use rustyline::Result;

/// Parsing and compiling deeply nested code recurses, which can exceed the
/// 1 MB default main-thread stack on Windows, so the REPL runs on a thread
/// with a larger stack.
const STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() -> Result<()> {
    std::thread::Builder::new()
        .name("pleno-repl".to_string())
        .stack_size(STACK_SIZE)
        .spawn(pleno_repl::start)?
        .join()
        .unwrap_or_else(|_| std::process::exit(101))
}
