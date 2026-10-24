use std::io;

/// Parsing and compiling deeply nested code recurses, which can exceed the
/// 1 MB default main-thread stack on Windows, so the server runs on a thread
/// with a larger stack.
const STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() -> io::Result<()> {
    std::thread::Builder::new()
        .name("pleno-lsp".to_string())
        .stack_size(STACK_SIZE)
        .spawn(pleno_lsp::start)?
        .join()
        .unwrap_or_else(|_| std::process::exit(101))
}
