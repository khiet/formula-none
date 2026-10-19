import { describe, expect, it } from "vitest";
import { parse } from "yaml";
import { listRepoDir, readRepoFile } from "./repo";

// The protocol's TypeScript types are generated from the Rust crate and
// committed; CI regenerates them and fails on a diff (ADR 0004).
interface Step {
  name?: string;
  run?: string;
}
const workflow = parse(readRepoFile(".github/workflows/ci.yml")) as {
  jobs: Record<string, { steps: Step[] }>;
};
const GENERATED = "apps/web/src/generated/protocol";

describe("protocol codegen", () => {
  it("is committed next to the bridge's gitignored output", () => {
    expect(listRepoDir(GENERATED)).toContain("index.ts");
    expect(listRepoDir(GENERATED)).toContain("version.ts");
    expect(listRepoDir(GENERATED)).toContain("fixtures.json");
    const ignored = readRepoFile(".gitignore")
      .split("\n")
      .filter((line) => line.trim() !== "" && !line.startsWith("#"));
    expect(ignored).toContain("apps/web/src/generated/bridge");
    expect(ignored).not.toContain("apps/web/src/generated");
  });

  it("is regenerated in CI and diffed against the committed output", () => {
    const steps = Object.values(workflow.jobs).flatMap((job) => job.steps);
    const check = steps.find((step) => step.run?.includes("pnpm generate:protocol"));
    expect(check?.run).toContain(`git diff --exit-code -- ${GENERATED}`);
  });

  it("has the package script CI and CLAUDE.md name", () => {
    const pkg = JSON.parse(readRepoFile("package.json")) as { scripts: Record<string, string> };
    expect(pkg.scripts["generate:protocol"]).toBeDefined();
    expect(readRepoFile("CLAUDE.md")).toContain("`pnpm generate:protocol`");
  });
});
