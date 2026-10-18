import { describe, expect, it } from "vitest";
import { parse } from "yaml";
import { readRepoFile } from "./repo";

// The deploy pipeline (ADR 0016) is configuration: the CI workflow's deploy
// job and the Compose stack the deploy document starts on the instance.
// These checks read both so a change that silently skips the gates, opens a
// port, or loses the certificate volume fails CI.
type Job = {
  needs?: string[];
  if?: string;
  permissions?: Record<string, string>;
  steps: Array<{ name?: string; uses?: string; run?: string; with?: Record<string, unknown> }>;
};

type Service = {
  image?: string;
  ports?: string[];
  volumes?: string[];
  restart?: string;
};

/** The workflow expression for the pushed commit, as the YAML spells it. */
const SHA = `\${{ github.sha }}`;

const workflow = parse(readRepoFile(".github/workflows/ci.yml")) as { jobs: Record<string, Job> };
const compose = parse(readRepoFile("deploy/compose.yaml")) as {
  name?: string;
  services: Record<string, Service>;
  volumes?: Record<string, unknown>;
};

describe("deploy job", () => {
  const { deploy, ...gates } = workflow.jobs;

  it("runs only for a push to main, after every other job has passed", () => {
    expect(deploy).toBeDefined();
    expect(deploy?.if).toContain("github.event_name == 'push'");
    expect(deploy?.if).toContain("github.ref == 'refs/heads/main'");
    expect([...(deploy?.needs ?? [])].sort()).toEqual(Object.keys(gates).sort());
  });

  it("assumes the deploy role through OIDC rather than stored keys", () => {
    expect(deploy?.permissions?.["id-token"]).toBe("write");
    const credentials = deploy?.steps.find((step) =>
      step.uses?.startsWith("aws-actions/configure-aws-credentials@"),
    );
    expect(credentials?.with?.["role-to-assume"]).toBeDefined();
    expect(JSON.stringify(deploy)).not.toMatch(/AWS_SECRET_ACCESS_KEY|aws-secret-access-key/);
  });

  it("builds an arm64 image tagged with the commit, then runs the one deploy script", () => {
    const build = deploy?.steps.find((step) => step.uses?.startsWith("docker/build-push-action@"));
    expect(build?.with?.platforms).toBe("linux/arm64");
    expect(build?.with?.tags).toContain(SHA);
    expect(build?.with?.["build-args"]).toContain(`COMMIT=${SHA}`);
    const run = deploy?.steps.find((step) => step.run?.includes("deploy/deploy.sh"));
    expect(run?.run?.trim()).toBe(`deploy/deploy.sh ${SHA}`);
  });
});

describe("Compose stack", () => {
  it("runs caddy and the server, both restarting with the instance", () => {
    expect(Object.keys(compose.services).sort()).toEqual(["caddy", "server"]);
    for (const service of Object.values(compose.services)) {
      expect(service.restart).toBe("unless-stopped");
    }
    // The project name fixes the volume names across deploys from any directory.
    expect(compose.name).toBe("formula-none");
  });

  it("publishes only 443 TCP, through caddy; the server stays internal", () => {
    expect(compose.services.caddy?.ports).toEqual(["443:443/tcp"]);
    expect(compose.services.server?.ports).toBeUndefined();
  });

  it("keeps the certificate in a named volume that outlives the containers", () => {
    const mounts = compose.services.caddy?.volumes ?? [];
    expect(mounts).toContain("caddy_data:/data");
    expect(Object.keys(compose.volumes ?? {})).toContain("caddy_data");
  });

  it("runs the image the deploy document names", () => {
    expect(compose.services.server?.image).toBe(`\${SERVER_IMAGE}`);
  });
});
