import { describe, expect, it } from "vitest";
import { listRepoDir, listRepoFiles, readRepoFile } from "./repo";

// The one-way dependency rule from ADR 0006, read off the manifests so a
// dependency in the wrong direction fails CI.
const ALLOWED_CRATE_DEPS: Record<string, string[]> = {
  sim: [],
  protocol: [],
  bridge: ["sim", "protocol"],
  server: ["sim", "protocol"],
};
const CRATES = Object.keys(ALLOWED_CRATE_DEPS);

// Crates that would tie the sim to a runtime or a platform (ADR 0002).
const SIM_FORBIDDEN_CRATES = ["tokio", "wasm-bindgen", "js-sys", "web-sys"];

/** The one web module allowed to import the generated wasm bindings (ADR 0003). */
const BRIDGE_WRAPPER = "apps/web/src/bridge.ts";

/**
 * Workspace crates a Cargo manifest names as a dependency, in any form Cargo
 * accepts: `sim = { path = ".." }`, `sim = { workspace = true }`, `sim.path`,
 * or a `[dependencies.sim]` table, under any dependency section.
 */
function workspaceDeps(manifest: string): string[] {
  const names = CRATES.join("|");
  const keyed = manifest.matchAll(new RegExp(`^(${names})\\s*[=.]`, "gm"));
  const tables = manifest.matchAll(new RegExp(`^\\[[\\w.-]*dependencies\\.(${names})\\]`, "gm"));
  return [...new Set([...keyed, ...tables].map((match) => match[1] ?? ""))];
}

describe("one-way dependency rule", () => {
  it("every workspace crate is covered by the rule", () => {
    expect(listRepoDir("crates")).toEqual([...CRATES].sort());
  });

  it.each(Object.entries(ALLOWED_CRATE_DEPS))(
    "%s depends only on %j inside the repo",
    (crate, allowed) => {
      const deps = workspaceDeps(readRepoFile(`crates/${crate}/Cargo.toml`));
      const forbidden = deps.filter((dep) => !allowed.includes(dep));
      expect(forbidden).toEqual([]);
    },
  );

  it.each(SIM_FORBIDDEN_CRATES)("sim does not depend on %s", (crate) => {
    const manifest = readRepoFile("crates/sim/Cargo.toml");
    expect(manifest).not.toMatch(new RegExp(`^${crate}\\s*[=.]|dependencies\\.${crate}\\]`, "m"));
  });

  it("only the wrapper module imports the generated bridge", () => {
    const importers = listRepoFiles("apps/web/src").filter(
      (file) =>
        !file.startsWith("apps/web/src/generated/") &&
        /from\s+["'][^"']*generated\/bridge\//.test(readRepoFile(file)),
    );
    expect(importers).toEqual([BRIDGE_WRAPPER]);
  });

  it("the web app reaches into the Cargo workspace only through the bridge", () => {
    const pkg = JSON.parse(readRepoFile("apps/web/package.json")) as {
      dependencies?: Record<string, string>;
      devDependencies?: Record<string, string>;
    };
    const deps = Object.entries({ ...pkg.dependencies, ...pkg.devDependencies });
    const intoCrates = deps.filter(
      ([name, spec]) => CRATES.includes(name) || /\bcrates\//.test(spec),
    );
    const notBridge = intoCrates.filter(
      ([name, spec]) => name !== "bridge" && !/crates\/bridge\b/.test(spec),
    );
    expect(notBridge).toEqual([]);
  });
});
