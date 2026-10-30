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
  environment?: Record<string, string>;
  user?: string;
  cap_drop?: string[];
  read_only?: boolean;
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
  it("runs caddy, the server, prometheus and grafana, all restarting with the instance", () => {
    expect(Object.keys(compose.services).sort()).toEqual([
      "caddy",
      "grafana",
      "prometheus",
      "server",
    ]);
    for (const service of Object.values(compose.services)) {
      expect(service.restart).toBe("unless-stopped");
    }
    // The project name fixes the volume names across deploys from any directory.
    expect(compose.name).toBe("formula-none");
  });

  // The security group opens 443 alone (infra.test.ts); the stack must not
  // rely on that by binding anything else to a public address.
  it("publishes 443 TCP through caddy and 443 UDP through the server, and grafana on the loopback only", () => {
    expect(compose.services.caddy?.ports).toEqual(["443:443/tcp"]);
    expect(compose.services.server?.ports).toEqual(["443:4433/udp"]);
    expect(compose.services.server?.environment?.WEBTRANSPORT_ADDR).toBe("0.0.0.0:4433");
    expect(compose.services.grafana?.ports).toEqual(["127.0.0.1:3000:3000"]);
    expect(compose.services.prometheus?.ports).toBeUndefined();
  });

  it("keeps the certificate in a named volume that outlives the containers", () => {
    const mounts = compose.services.caddy?.volumes ?? [];
    expect(mounts).toContain("caddy_data:/data");
    expect(Object.keys(compose.volumes ?? {})).toContain("caddy_data");
  });

  // The server terminates QUIC with the certificate Caddy obtained (ADR
  // 0011): it reads Caddy's volume, never a copy, so a renewal reaches it.
  it("hands the server Caddy's certificate files read-only, with no capabilities", () => {
    const server = compose.services.server;
    expect(server?.volumes).toContain("caddy_data:/data:ro");
    const site = "formulanone.sushimafia.com";
    const issuer = "/data/caddy/certificates/acme-v02.api.letsencrypt.org-directory";
    const expand = (value: string | undefined) =>
      value?.replaceAll(`\${SITE_ADDRESS:-${site}}`, site);
    expect(expand(server?.environment?.TLS_CERT)).toBe(`${issuer}/${site}/${site}.crt`);
    expect(expand(server?.environment?.TLS_KEY)).toBe(`${issuer}/${site}/${site}.key`);
    // Caddy's key is root-only, so the server runs as root with nothing else.
    expect(server?.user).toBe("0:0");
    expect(server?.cap_drop).toEqual(["ALL"]);
    expect(server?.read_only).toBe(true);
  });

  // Open rooms survive a restart (ADR 0005): the server writes them under
  // ROOMS_DIR, which is a named volume, the one writable path in a
  // read-only container.
  it("keeps open rooms in a named volume that outlives the server container", () => {
    const server = compose.services.server;
    const dir = server?.environment?.ROOMS_DIR;
    expect(dir).toBeDefined();
    const mount = (server?.volumes ?? []).find((volume) => {
      const [name, path] = volume.split(":");
      return name === "rooms" && dir?.startsWith(`${path}/`);
    });
    expect(mount).toBe("rooms:/var/lib/formula-none");
    expect(Object.keys(compose.volumes ?? {})).toContain("rooms");
  });

  it("runs the image the deploy document names", () => {
    expect(compose.services.server?.image).toBe(`\${SERVER_IMAGE}`);
  });
});

// Metrics (ADR 0018): Prometheus scrapes the server over the Compose
// network and Grafana reads Prometheus; the dashboard and the data source
// are files in the repo, mounted into Grafana's provisioning directory.
describe("metrics stack", () => {
  const prometheus = parse(readRepoFile("deploy/prometheus.yml")) as {
    scrape_configs: Array<{
      job_name: string;
      metrics_path?: string;
      static_configs: Array<{ targets: string[] }>;
    }>;
  };
  const datasource = parse(
    readRepoFile("deploy/grafana/provisioning/datasources/prometheus.yml"),
  ) as {
    datasources: Array<{
      name: string;
      type: string;
      uid: string;
      url: string;
      isDefault?: boolean;
    }>;
  };
  const provider = parse(readRepoFile("deploy/grafana/provisioning/dashboards/dashboards.yml")) as {
    providers: Array<{ options: { path: string } }>;
  };
  const dashboard = JSON.parse(readRepoFile("deploy/grafana/dashboards/netcode.json")) as {
    uid: string;
    title: string;
    panels: Array<{
      title: string;
      datasource?: { uid: string };
      targets: Array<{ expr: string; datasource?: { uid: string } }>;
    }>;
  };

  it("scrapes the server's metrics endpoint, which Caddy does not proxy", () => {
    const [job, ...others] = prometheus.scrape_configs;
    expect(others).toEqual([]);
    expect(job?.static_configs.flatMap((config) => config.targets)).toEqual(["server:8080"]);
    const path = job?.metrics_path ?? "/metrics";
    expect(path).toBe("/metrics");
    // Caddy forwards /api/* alone, so the scrape path stays on the Compose network.
    const caddyfile = readRepoFile("deploy/Caddyfile");
    expect(caddyfile.match(/reverse_proxy/g)).toHaveLength(1);
    expect(caddyfile).toMatch(/handle \/api\/\* \{\s*reverse_proxy server:8080/);
  });

  it("keeps Prometheus's data in a named volume, bounded in time and size", () => {
    const service = compose.services.prometheus;
    expect(service?.volumes).toContain("./prometheus.yml:/etc/prometheus/prometheus.yml:ro");
    expect(service?.volumes).toContain("prometheus_data:/prometheus");
    expect(Object.keys(compose.volumes ?? {})).toContain("prometheus_data");
    const command = (service as { command?: string[] } | undefined)?.command ?? [];
    expect(command.some((flag) => flag.startsWith("--storage.tsdb.retention.time="))).toBe(true);
    expect(command.some((flag) => flag.startsWith("--storage.tsdb.retention.size="))).toBe(true);
  });

  it("provisions Grafana's data source and dashboard from the repo, with no login behind the SSM port forward", () => {
    const grafana = compose.services.grafana;
    expect(grafana?.volumes).toContain("./grafana/provisioning:/etc/grafana/provisioning:ro");
    const [source, ...otherSources] = datasource.datasources;
    expect(otherSources).toEqual([]);
    expect(source?.type).toBe("prometheus");
    expect(source?.url).toBe("http://prometheus:9090");
    expect(source?.isDefault).toBe(true);
    const [dashboards, ...otherProviders] = provider.providers;
    expect(otherProviders).toEqual([]);
    expect(grafana?.volumes).toContain(`./grafana/dashboards:${dashboards?.options.path}:ro`);
    // Session Manager already authenticated whoever reaches the port.
    expect(grafana?.environment?.GF_AUTH_ANONYMOUS_ENABLED).toBe("true");
    expect(grafana?.environment?.GF_AUTH_ANONYMOUS_ORG_ROLE).toBe("Admin");
    expect(grafana?.environment?.GF_AUTH_DISABLE_LOGIN_FORM).toBe("true");
  });

  it("draws the five metrics of ADR 0018, one panel each, from the provisioned data source", () => {
    const [source] = datasource.datasources;
    expect(dashboard.panels).toHaveLength(5);
    const metricOf = (panel: (typeof dashboard.panels)[number]) =>
      panel.targets.map((target) => target.expr).join(" ");
    const exprs = dashboard.panels.map(metricOf);
    const names = [
      "formula_none_tick_duration_seconds",
      "formula_none_room_players",
      "formula_none_client_rtt_seconds",
      "formula_none_client_packet_loss",
      "formula_none_client_corrections_per_second",
    ];
    for (const name of names) {
      expect(exprs.filter((expr) => expr.includes(name))).toHaveLength(1);
    }
    for (const panel of dashboard.panels) {
      expect(panel.datasource?.uid).toBe(source?.uid);
      for (const target of panel.targets) {
        expect(target.datasource?.uid ?? source?.uid).toBe(source?.uid);
      }
    }
  });

  it("ships the Prometheus config and Grafana's files in the release bundle the instance runs from", () => {
    const dockerfile = readRepoFile("Dockerfile");
    expect(dockerfile).toMatch(
      /^COPY deploy\/compose\.yaml deploy\/Caddyfile deploy\/prometheus\.yml \/release\/$/m,
    );
    expect(dockerfile).toMatch(/^COPY deploy\/grafana \/release\/grafana$/m);
  });
});
