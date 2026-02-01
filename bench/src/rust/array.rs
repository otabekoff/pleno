use std::time::Instant;

fn main() {
    let start = Instant::now();

    // Create 1M vector
    let arr: Vec<u64> = (0..1000000).collect();

    // Map, Filter, Reduce using iterators (idiomatic)
    let sum: u64 = arr.iter()
        .map(|x| x * 2)
        .filter(|x| x % 3 == 0)
        .sum();

    let duration = start.elapsed();
    
    println!("Rust (Array Ops 1M): {}", sum);
    println!("Time: {:.2}ms", duration.as_secs_f64() * 1000.0);
}
