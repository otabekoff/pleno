fn main() {
    let debug = std::env::args().any(|arg| arg == "--debug" || arg == "-d");
    pleno_common::logging::init(debug);
    pleno_cli::run_with_large_stack(pleno_cli::run_pleno);
}
