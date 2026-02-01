use std::time::Instant;

fn main() {
    let start = Instant::now();
    let mut sum: u64 = 0;
    for i in 0..10000000 {
        sum += std::hint::black_box(i);
    }
    let duration = start.elapsed();

    println!("Rust (Loop Sum 10M): {}", sum);
    println!("Time: {:.2}ms", duration.as_secs_f64() * 1000.0);
}
