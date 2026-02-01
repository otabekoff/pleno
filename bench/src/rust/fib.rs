use std::time::Instant;

fn fib(n: u64) -> u64 {
    if n < 2 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn main() {
    let start = Instant::now();
    let res = fib(30);
    let duration = start.elapsed();

    println!("Rust (Recursive Fib 30): {}", res);
    println!("Time: {:.2}ms", duration.as_secs_f64() * 1000.0);
}
