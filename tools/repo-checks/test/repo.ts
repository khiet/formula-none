import { readdirSync, readFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";

/** Repository root; this package lives at tools/repo-checks. */
export const ROOT = resolve(import.meta.dirname, "../../..");

export function readRepoFile(relativePath: string): string {
  return readFileSync(join(ROOT, relativePath), "utf8");
}

/** A prose file with every run of whitespace folded to one space, so a sentence matches however it wraps. */
export function readRepoProse(relativePath: string): string {
  return readRepoFile(relativePath).replace(/\s+/g, " ");
}

export function listRepoDir(relativePath: string): string[] {
  return readdirSync(join(ROOT, relativePath)).sort();
}

/** Every file under a directory, recursively, as repo-relative paths in sorted order. */
export function listRepoFiles(relativePath: string): string[] {
  return readdirSync(join(ROOT, relativePath), { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => relative(ROOT, join(entry.parentPath, entry.name)))
    .sort();
}
