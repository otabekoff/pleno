function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

const start = performance.now();
const result = fib(30);
const end = performance.now();

console.log(`Fibonacci(30) = ${result}`);
console.log(`Time: ${Math.round(end - start)}ms`);
