import { execFile } from "node:child_process";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { ROOT } from "./repo";

// The smoke check is what makes a deploy job fail (ADR 0016). It is run
// here against a stand-in for the deployed stack: a base URL that answers
// the client, health and version the way Caddy and the server do.
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
      { encoding: "utf8", env: { ...process.env, SMOKE_TIMEOUT: "2" } },
      (error, stdout, stderr) => {
        const status = error ? (typeof error.code === "number" ? error.code : null) : 0;
        resolve({ status, output: stdout + stderr });
      },
    );
  });
}

afterEach(async () => {
  await new Promise((resolve) => server?.close(resolve));
  server = undefined;
});

describe("deploy/smoke.sh", () => {
  it("passes when the client, health and the deployed commit all answer", async () => {
    const result = await smoke(await listen(stack(COMMIT)));
    expect(result.output).toContain(COMMIT);
    expect(result.status).toBe(0);
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
