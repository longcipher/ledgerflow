import { readFileSync } from "node:fs";
import { join } from "node:path";

export const packageInfo = JSON.parse(readFileSync(join(__dirname, "../../package.json"), "utf-8"));
