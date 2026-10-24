use anyhow::Result;
use pleno_dap::server::DapServer;
use std::io::{BufReader, stdout};

/// Parsing and compiling deeply nested code recurses, which can exceed the
/// 1 MB default main-thread stack on Windows, so the adapter runs on a thread
/// with a larger stack.
const STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() -> Result<()> {
    std::thread::Builder::new()
        .name("pleno-dap".to_string())
        .stack_size(STACK_SIZE)
        .spawn(run)?
        .join()
        .unwrap_or_else(|_| std::process::exit(101))
}

fn run() -> Result<()> {
    let stdin = std::io::stdin();
    let input = BufReader::new(stdin);
    let output = stdout();

    let mut server = DapServer::new(input, output);
    server.run()
}
