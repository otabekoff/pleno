use anyhow::Result;
use pleno_dap::server::DapServer;
use std::io::{BufReader, stdout};

fn main() -> Result<()> {
    let stdin = std::io::stdin();
    let input = BufReader::new(stdin);
    let output = stdout();

    let mut server = DapServer::new(input, output);
    server.run()
}
