use std::io::Write;

pub fn init(debug: bool) {
    let mut builder = env_logger::Builder::from_default_env();

    if debug {
        builder.filter_level(log::LevelFilter::Debug);
    } else {
        // Only show warnings and errors in production mode for internal crates
        // unless overridden by RUST_LOG
        if std::env::var("RUST_LOG").is_err() {
            builder.filter_module("pleno", log::LevelFilter::Info);
        }
    }

    builder
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] {:<5} - {}",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();
}
