import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));

export const packageInfo = JSON.parse(readFileSync(join(__dirname, "../../package.json"), "utf-8"));
