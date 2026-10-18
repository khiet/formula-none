import { describe, expect, it } from "vitest";
import { listRepoDir, readRepoFile } from "./repo";

// The one-way dependency rule from ADR 0006, read off the manifests so a
// dependency in the wrong direction fails the build.
const ALLOWED_CRATE_DEPS: Record<string, string[]> = {
  sim: [],
  protocol: [],
  bridge: ["sim", "protocol"],
  server: ["sim", "protocol"],
};

/** Names of workspace crates a Cargo manifest depends on via `path = ...`. */
function workspaceDeps(manifest: string): string[] {
  return [...manifest.matchAll(/^(\w+)\s*=\s*\{[^}]*\bpath\s*=/gm)].map((match) => match[1] ?? "");
}

describe("one-way dependency rule", () => {
  it("every workspace crate is covered by the rule", () => {
    expect(listRepoDir("crates")).toEqual(Object.keys(ALLOWED_CRATE_DEPS).sort());
  });

  it.each(Object.entries(ALLOWED_CRATE_DEPS))(
    "%s depends only on %j inside the repo",
    (crate, allowed) => {
      const deps = workspaceDeps(readRepoFile(`crates/${crate}/Cargo.toml`));
      const forbidden = deps.filter((dep) => !allowed.includes(dep));
      expect(forbidden).toEqual([]);
    },
  );

  it("the web app depends on no workspace crate other than the bridge", () => {
    const pkg = JSON.parse(readRepoFile("apps/web/package.json")) as {
      dependencies?: Record<string, string>;
      devDependencies?: Record<string, string>;
    };
    const deps = Object.keys({ ...pkg.dependencies, ...pkg.devDependencies });
    const crates = ["sim", "protocol", "server"];
    expect(deps.filter((dep) => crates.includes(dep))).toEqual([]);
  });
});
