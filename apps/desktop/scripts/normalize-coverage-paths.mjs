import { readFile, writeFile } from "node:fs/promises";

const sourcePath = new URL("../coverage/coverage-final.json", import.meta.url);
const destinationPath = new URL("../coverage/coverage-final.normalized.json", import.meta.url);
const coverage = JSON.parse(await readFile(sourcePath, "utf8"));
const normalizedCoverage = Object.fromEntries(
  Object.entries(coverage).map(([path, entry]) => {
    const normalizedPath = path.replaceAll("\\", "/");
    return [normalizedPath, { ...entry, path: normalizedPath }];
  }),
);

await writeFile(destinationPath, `${JSON.stringify(normalizedCoverage)}\n`);
