import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

/** Repository root; this package lives at tools/repo-checks. */
export const ROOT = resolve(import.meta.dirname, "../../..");

export function readRepoFile(relativePath: string): string {
  return readFileSync(join(ROOT, relativePath), "utf8");
}

export function listRepoDir(relativePath: string): string[] {
  return readdirSync(join(ROOT, relativePath)).sort();
}
