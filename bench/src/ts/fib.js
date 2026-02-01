function fib(n) {
    if (n < 2)
        return n;
    return fib(n - 1) + fib(n - 2);
}
var start = performance.now();
var res = fib(30);
var duration = performance.now() - start;
console.log("TypeScript (Recursive Fib 30): ".concat(res));
console.log("Time: ".concat(duration.toFixed(2), "ms"));
