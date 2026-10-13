import assert from "node:assert/strict";

// Ordinary JSON.parse has already lost precision before conversion to bigint.
const raw = JSON.parse("18446744073709551615");
assert.equal(BigInt(raw), 18446744073709551616n);
assert.notEqual(BigInt(raw), 18446744073709551615n);

// A string wire representation preserves the integer's digits.
const exact = JSON.parse('"18446744073709551615"');
assert.equal(BigInt(exact), 18446744073709551615n);

// Declaring a TypeScript field as bigint would not supply a JSON codec for it.
assert.throws(() => JSON.stringify(18446744073709551615n), TypeError);

console.log("JSON number precision and bigint checks passed");
