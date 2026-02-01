const start = performance.now();

// Create 1M array
const arr: number[] = new Array(1000000);
for (let i = 0; i < 1000000; i++) {
    arr[i] = i;
}

// Map * 2
const mapped = arr.map(x => x * 2);

// Filter % 3
const filtered = mapped.filter(x => x % 3 === 0);

// Reduce
const sum = filtered.reduce((acc, x) => acc + x, 0);

const duration = performance.now() - start;

console.log(`TypeScript (Array Ops 1M): ${sum}`);
console.log(`Time: ${duration.toFixed(2)}ms`);
export { };
