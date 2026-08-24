import { mkdir } from "node:fs/promises";

await mkdir("target/crap", { recursive: true });
