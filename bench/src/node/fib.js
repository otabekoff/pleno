function fib(n) {
    if (n < 2) return n;
    return fib(n - 1) + fib(n - 2);
}

const start = performance.now();
const res = fib(30);
const duration = performance.now() - start;

console.log(`Node.js (Recursive Fib 30): ${res}`);
console.log(`Time: ${duration.toFixed(2)}ms`);
