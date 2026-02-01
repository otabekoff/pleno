let sum = 0;
const start = performance.now();
for (let i = 0; i < 1000000; i++) {
    sum += i;
}
const end = performance.now();
console.log(`Loop Sum = ${sum}`);
console.log(`Time: ${Math.round(end - start)}ms`);
