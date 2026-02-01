const start = performance.now();
let sum = 0;
for (let i = 0; i < 10000000; i++) {
    sum += i;
}
const duration = performance.now() - start;

console.log(`TypeScript (Loop Sum 10M): ${sum}`);
console.log(`Time: ${duration.toFixed(2)}ms`);
export {};
