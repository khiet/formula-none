import { existsSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { ROOT, readRepoFile, readRepoProse } from "./repo";

// The repository is public (ADR 0066): the README is its front page, and
// CONTRIBUTING.md and SECURITY.md are the files GitHub shows a visitor who
// wants to help or to report a hole.
describe("README", () => {
  const readme = readRepoProse("README.md");

  it("opens on the Race View clip, a file the repo holds and licenses", () => {
    // The licence badge comes first; it is shields.io's image, not the repo's.
    const [, first] = /!\[[^\]]*\]\((?!https?:)([^)]+)\)/.exec(readme) ?? [];
    expect(first).toBe("docs/race-view.gif");
    expect(existsSync(join(ROOT, "docs/race-view.gif"))).toBe(true);
    expect(readRepoFile("assets/LICENSES.md")).toMatch(/`docs\/\*\*\/\*\.gif`.*Race View clip/);
  });

  it("links the hosted instance", () => {
    expect(readme).toContain("https://formulanone.sushimafia.com");
  });

  it("states the code and art licences, pointing at the files that hold them", () => {
    expect(readme).toContain("The code is [MIT](LICENSE).");
    expect(readme).toMatch(/The art and sound are CC0 1\.0.*\(assets\/LICENSES\.md\)/);
  });

  it("disclaims any link to Formula One or a real circuit", () => {
    expect(readme).toContain("not affiliated with Formula One or any circuit");
  });

  it("sends self-hosters to deploy/, since infra/ is the author's own deployment", () => {
    expect(readme).toMatch(
      /`infra\/` describes the author's own deployment.*Compose stack under `deploy\/`/,
    );
    expect(readme).toContain("(deploy/README.md#hosting-your-own)");
    expect(readRepoFile("deploy/README.md")).toMatch(/^## Hosting your own$/m);
  });
});

describe("CONTRIBUTING.md", () => {
  const contributing = readRepoProse("CONTRIBUTING.md");

  it("names the help wanted label, the conventions in CLAUDE.md and the pnpm check gate", () => {
    expect(contributing).toContain("`help wanted`");
    expect(contributing).toContain("`pnpm check`");
    expect(contributing).toContain("CLAUDE.md");
  });
});

describe("SECURITY.md", () => {
  const security = readRepoProse("SECURITY.md");

  // GitHub's private reporting needs no address, so the file publishes none.
  it("routes reports through GitHub's private vulnerability reporting, with no address", () => {
    expect(security).toContain("/security/advisories/new");
    expect(security).not.toMatch(/[\w.+-]+@[\w-]+\.[\w.]+/);
  });
});
