import { execFile } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ROOT } from "./repo";

// The smoke check is what makes a deploy job fail (ADR 0016). It is run
// here against a stand-in for the deployed stack: a base URL that answers
// the client, health and version the way Caddy and the server do, and a
// stand-in for the WebTransport probe that records the URL it was given
// and exits as told.
const COMMIT = "0123abcd";

type Routes = Record<string, { status: number; type: string; body: string }>;

function stack(commit: string): Routes {
  return {
    "/": { status: 200, type: "text/html; charset=utf-8", body: "<!doctype html><title>x</title>" },
    "/api/health": { status: 200, type: "application/json", body: '{"status":"ok"}' },
    "/api/version": { status: 200, type: "application/json", body: `{"commit":"${commit}"}` },
  };
}

let server: Server | undefined;
let probeDir: string;
let probe: string;

/** A probe script that logs its argument and exits with `status`. */
function fakeProbe(status: number): string {
  const script = join(probeDir, "probe.sh");
  writeFileSync(script, `#!/bin/sh\necho "$1" >> "${probeDir}/calls"\nexit ${status}\n`);
  chmodSync(script, 0o755);
  return script;
}

function probeCalls(): string[] {
  try {
    return readFileSync(join(probeDir, "calls"), "utf8").trim().split("\n");
  } catch {
    return [];
  }
}

async function listen(routes: Routes): Promise<string> {
  server = createServer((req, res) => {
    const route = routes[req.url ?? ""];
    if (!route) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(route.status, { "content-type": route.type }).end(route.body);
  });
  await new Promise<void>((resolve) => server?.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as AddressInfo;
  return `http://127.0.0.1:${port}`;
}

// Asynchronous so the stand-in server in this process keeps answering.
function smoke(baseUrl: string): Promise<{ status: number | null; output: string }> {
  return new Promise((resolve) => {
    execFile(
      join(ROOT, "deploy/smoke.sh"),
      [COMMIT, baseUrl],
      // Short budget: these runs either answer at once or never will.
      {
        encoding: "utf8",
        env: { ...process.env, SMOKE_TIMEOUT: "2", SMOKE_WEBTRANSPORT_PROBE: probe },
      },
      (error, stdout, stderr) => {
        const status = error ? (typeof error.code === "number" ? error.code : null) : 0;
        resolve({ status, output: stdout + stderr });
      },
    );
  });
}

beforeEach(() => {
  probeDir = mkdtempSync(join(tmpdir(), "smoke-"));
  probe = fakeProbe(0);
});

afterEach(async () => {
  await new Promise((resolve) => server?.close(resolve));
  server = undefined;
  rmSync(probeDir, { recursive: true, force: true });
});

describe("deploy/smoke.sh", () => {
  it("passes when the client, health, the deployed commit and the WebTransport handshake all answer", async () => {
    const base = await listen(stack(COMMIT));
    const result = await smoke(base);
    expect(result.output).toContain(COMMIT);
    expect(result.status).toBe(0);
    // One handshake, at the path the client connects to, on the site's own port.
    expect(probeCalls()).toEqual([`${base}/wt`]);
  });

  it("fails when the WebTransport handshake fails", async () => {
    probe = fakeProbe(1);
    const result = await smoke(await listen(stack(COMMIT)));
    expect(result.status).not.toBe(0);
    expect(result.output).toContain("webtransport handshake failed");
  });

  it("fails when the version endpoint reports another commit", async () => {
    const result = await smoke(await listen(stack("ffffffff")));
    expect(result.status).not.toBe(0);
    expect(result.output).toContain("ffffffff");
  });

  it("fails when health does not answer", async () => {
    const routes = stack(COMMIT);
    delete routes["/api/health"];
    const result = await smoke(await listen(routes));
    expect(result.status).not.toBe(0);
  });

  it("fails when nothing listens at all", async () => {
    const base = await listen({});
    await new Promise((resolve) => server?.close(resolve));
    const result = await smoke(base);
    expect(result.status).not.toBe(0);
  });
});
