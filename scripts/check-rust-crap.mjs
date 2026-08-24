import { readFile } from "node:fs/promises";

const threshold = 30;
const report = JSON.parse(await readFile("target/crap/rust-crap.json", "utf8"));
const failures = report.entries.filter(
  (entry) => entry.status === "regressed" || (entry.status === "new" && entry.crap > threshold),
);

if (failures.length === 0) {
  console.log(`Rust CRAP gate passed: no regressions or new functions above ${threshold}.`);
  process.exit(0);
}

console.error(`Rust CRAP gate failed: ${failures.length} function(s) need attention.`);
for (const entry of failures) {
  console.error(
    `${entry.status}: ${entry.function} at ${entry.file}:${entry.line} (CRAP ${entry.crap.toFixed(1)})`,
  );
}
process.exit(1);
