import { describe, expect, it } from "vitest";
import { readRepoProse } from "./repo";

// CLAUDE.md is the first file an agent reads. It must carry the one-way
// dependency rule and the one command each for build, lint and test on both
// sides, so nobody has to rediscover them.
const COMMANDS = [
  "cargo build --workspace",
  "cargo clippy --all-targets -- -D warnings",
  "cargo nextest run",
  "pnpm build",
  "pnpm lint",
  "pnpm test",
];

describe("CLAUDE.md", () => {
  // Prose wraps at 80 columns; the rule is a sentence, not a line.
  const claudeMd = readRepoProse("CLAUDE.md");

  it("states the one-way dependency rule", () => {
    expect(claudeMd).toContain("web depends on the bridge");
    expect(claudeMd).toContain("the bridge and the server depend on sim and protocol");
    expect(claudeMd).toContain("sim depends on nothing in the repo");
  });

  it.each(COMMANDS)("documents `%s`", (command) => {
    expect(claudeMd).toContain(`\`${command}\``);
  });
});
