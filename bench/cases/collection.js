const start = process.hrtime.bigint();

// Collection operations benchmark
const arr = [];
for (let i = 0; i < 10000; i++) {
    arr.push(i);
}

// Map
const doubled = arr.map(x => x * 2);

// Filter
const evens = doubled.filter(x => x % 4 === 0);

// Reduce
const sum = evens.reduce((acc, x) => acc + x, 0);

const end = process.hrtime.bigint();
console.log("Original length: " + arr.length);
console.log("Doubled length: " + doubled.length);
console.log("Evens length: " + evens.length);
console.log("Sum: " + sum);
console.log("Time: " + Number(end - start) / 1000000 + "ms");
