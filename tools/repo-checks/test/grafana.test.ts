import { execFileSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ROOT, readRepoFile } from "./repo";

// Grafana is reached through an SSM port forward and nothing else (ADR
// 0015, ADR 0018): one script opens it. Run here against a stand-in `aws`
// that records what it was asked.
let binDir: string;

beforeEach(() => {
  binDir = mkdtempSync(join(tmpdir(), "grafana-"));
  const aws = join(binDir, "aws");
  writeFileSync(aws, `#!/bin/sh\nprintf '%s\\n' "$@" > "${binDir}/args"\n`);
  chmodSync(aws, 0o755);
});

afterEach(() => {
  rmSync(binDir, { recursive: true, force: true });
});

describe("deploy/grafana.sh", () => {
  it("forwards local port 3000 to Grafana on the instance through Session Manager", () => {
    execFileSync(join(ROOT, "deploy/grafana.sh"), {
      env: { ...process.env, PATH: `${binDir}:${process.env.PATH}`, INSTANCE_ID: "i-0123" },
    });
    const args = readFileSync(join(binDir, "args"), "utf8").trim().split("\n");
    expect(args.slice(0, 2)).toEqual(["ssm", "start-session"]);
    expect(args).toContain("i-0123");
    expect(args[args.indexOf("--document-name") + 1]).toBe("AWS-StartPortForwardingSession");
    const parameters = JSON.parse(args[args.indexOf("--parameters") + 1] ?? "{}") as Record<
      string,
      string[]
    >;
    expect(parameters).toEqual({ portNumber: ["3000"], localPortNumber: ["3000"] });
  });

  it("is the one command both READMEs point at", () => {
    expect(readRepoFile("deploy/README.md")).toContain("deploy/grafana.sh");
    expect(readRepoFile("infra/README.md")).toContain("deploy/grafana.sh");
  });
});
