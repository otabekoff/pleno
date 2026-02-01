const start = process.hrtime.bigint();

// String operations benchmark
let result = "";
for (let i = 0; i < 10000; i++) {
    result = result + "x";
}

const upper = result.toUpperCase();
const replaced = upper.replace(/X/g, "Y");
const parts = replaced.split("Y");

const end = process.hrtime.bigint();
console.log("String length: " + result.length);
console.log("Parts count: " + parts.length);
console.log("Time: " + Number(end - start) / 1000000 + "ms");
