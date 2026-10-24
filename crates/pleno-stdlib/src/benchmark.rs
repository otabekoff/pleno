use pleno_value::{RuntimeError, VMProvider, Value};
use std::time::Instant;

/// `Benchmark.run(name, fn, iterations = 1)`: calls `fn` `iterations` times,
/// prints the average duration and returns it in nanoseconds.
pub fn benchmark_run(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let name = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        _ => return Err(RuntimeError::new("Benchmark.run expects a name")),
    };
    let callee = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("Benchmark.run expects a function"))?;
    let iterations = match args.get(2) {
        None => 1,
        Some(Value::Int(n)) if *n > 0 => *n as u64,
        Some(_) => {
            return Err(RuntimeError::new(
                "Benchmark.run iterations must be a positive integer",
            ));
        }
    };

    let start = Instant::now();
    for _ in 0..iterations {
        vm.call(callee.clone(), Vec::new())?;
    }
    let average_ns = (start.elapsed().as_nanos() / iterations as u128).max(1) as i64;

    vm.print(&format!(
        "{name}: {average_ns} ns/iter ({iterations} iterations)\n"
    ));
    Ok(Value::Int(average_ns))
}
