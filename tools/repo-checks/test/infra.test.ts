import { parse } from "@cdktf/hcl2json";
import { beforeAll, describe, expect, it } from "vitest";
import { listRepoDir, readRepoFile } from "./repo";

// The whole environment is one OpenTofu root module under infra/ (ADR 0015).
// These checks read the configuration itself, since applying it needs the
// personal AWS account and runs by hand.
type Block = Record<string, unknown>;
type Resources = Record<string, Record<string, Block[]>>;

/** How hcl2json renders an unevaluated reference, so expected values read as HCL. */
function ref(expression: string): string {
  return `\${${expression}}`;
}

let resources: Resources;
let dataSources: Resources;
let variables: Record<string, Block>;

/** Resolve a plain `${var.name}` reference to the variable's default; other values pass through. */
function resolved(value: unknown): unknown {
  if (typeof value !== "string") return value;
  const match = /^\$\{var\.(\w+)\}$/.exec(value);
  return match ? variables[match[1] as string]?.default : value;
}

function only<T>(items: T[]): T {
  expect(items).toHaveLength(1);
  return items[0] as T;
}

function resourcesOfType(type: string): Record<string, Block> {
  return Object.fromEntries(
    Object.entries(resources[type] ?? {}).map(([name, blocks]) => [name, only(blocks)]),
  );
}

function policyDocument(name: string): Block[] {
  const document = only(dataSources.aws_iam_policy_document?.[name] ?? []);
  return document.statement as Block[];
}

beforeAll(async () => {
  const files = listRepoDir("infra").filter((name) => name.endsWith(".tf"));
  const parsed = await Promise.all(files.map((file) => parse(file, readRepoFile(`infra/${file}`))));
  resources = {};
  dataSources = {};
  variables = {};
  for (const module of parsed as Array<{
    resource?: Resources;
    data?: Resources;
    variable?: Record<string, Block[]>;
  }>) {
    for (const [type, byName] of Object.entries(module.data ?? {})) {
      dataSources[type] = { ...dataSources[type], ...byName };
    }
    for (const [name, blocks] of Object.entries(module.variable ?? {})) {
      variables[name] = only(blocks);
    }
    for (const [type, byName] of Object.entries(module.resource ?? {})) {
      resources[type] = { ...resources[type], ...byName };
    }
  }
});

describe("infra", () => {
  it("opens exactly 443 TCP and 443 UDP from anywhere, and nothing else", () => {
    const ingress = Object.values(resourcesOfType("aws_vpc_security_group_ingress_rule"));
    const rules = ingress
      .map((rule) => [rule.ip_protocol, rule.from_port, rule.to_port, rule.cidr_ipv4].join(" "))
      .sort();
    expect(rules).toEqual(["tcp 443 443 0.0.0.0/0", "udp 443 443 0.0.0.0/0"]);
  });

  it("runs one t4g.small reached through SSM Session Manager, with no SSH key pair", () => {
    const instance = only(Object.values(resourcesOfType("aws_instance")));
    expect(resolved(instance.instance_type)).toBe("t4g.small");
    expect(instance.key_name).toBeUndefined();
    expect(resourcesOfType("aws_key_pair")).toEqual({});
    // Session Manager needs the agent's managed policy on the instance role.
    const attachments = Object.values(resourcesOfType("aws_iam_role_policy_attachment"));
    expect(attachments.map((a) => a.policy_arn)).toContain(
      "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore",
    );
  });

  it("keeps container logs for 14 days", () => {
    const group = only(Object.values(resourcesOfType("aws_cloudwatch_log_group")));
    expect(resolved(group.retention_in_days)).toBe(14);
  });

  it("points formulanone.sushimafia.com at the Elastic IP from the zone's own account", () => {
    const record = only(Object.values(resourcesOfType("aws_route53_record")));
    expect(resolved(record.name)).toBe("formulanone.sushimafia.com");
    expect(record.type).toBe("A");
    expect(record.records).toEqual([ref("aws_eip.server.public_ip")]);
    expect(record.provider).toBe(ref("aws.dns"));
  });

  describe("GitHub Actions role", () => {
    // Every statement across the role's own policy documents, flattened.
    function statementsOf(role: Block): Block[] {
      const policies = Object.values(resourcesOfType("aws_iam_role_policy")).filter(
        (policy) => policy.role === ref(`aws_iam_role.${role.__name}.id`),
      );
      return policies.flatMap((policy) => {
        const match = /^\$\{data\.aws_iam_policy_document\.(\w+)\.json\}$/.exec(
          policy.policy as string,
        );
        expect(match).not.toBeNull();
        return policyDocument(match?.[1] as string);
      });
    }

    function deployRole(): Block {
      const [name, role] = only(
        Object.entries(resourcesOfType("aws_iam_role")).filter(([, role]) =>
          (role.assume_role_policy as string).includes("github"),
        ),
      );
      return { ...role, __name: name };
    }

    it("is assumable only by this repository's main branch through GitHub's OIDC provider", () => {
      const trust = policyDocument("github_trust");
      const statement = only(trust);
      expect(statement.actions).toEqual(["sts:AssumeRoleWithWebIdentity"]);
      const principal = only(statement.principals as Block[]);
      expect(principal.type).toBe("Federated");
      expect(principal.identifiers).toEqual([ref("aws_iam_openid_connect_provider.github.arn")]);
      const conditions = Object.fromEntries(
        (statement.condition as Block[]).map((c) => [c.variable, c.values]),
      );
      expect(conditions["token.actions.githubusercontent.com:aud"]).toEqual(["sts.amazonaws.com"]);
      expect(conditions["token.actions.githubusercontent.com:sub"]).toEqual([
        `repo:${ref("var.github_repository")}:ref:refs/heads/main`,
      ]);
      expect(resolved(ref("var.github_repository"))).toBe("khiet/formula-none");
    });

    it("may push to the one repository and run the one deploy document, and nothing else", () => {
      const statements = statementsOf(deployRole());
      const byAction = new Map<string, unknown>();
      for (const statement of statements) {
        for (const action of statement.actions as string[])
          byAction.set(action, statement.resources);
      }
      expect([...byAction.keys()].sort()).toEqual([
        "ecr:BatchCheckLayerAvailability",
        "ecr:BatchGetImage",
        "ecr:CompleteLayerUpload",
        "ecr:GetAuthorizationToken",
        "ecr:InitiateLayerUpload",
        "ecr:PutImage",
        "ecr:UploadLayerPart",
        "ssm:GetCommandInvocation",
        "ssm:SendCommand",
      ]);
      // The document the role may run is the only one, and it is the deploy.
      const [documentName, document] = only(Object.entries(resourcesOfType("aws_ssm_document")));
      expect(documentName).toBe("deploy");
      expect(document.document_type).toBe("Command");
      // GetAuthorizationToken takes no resource; everything else is pinned.
      expect(byAction.get("ecr:GetAuthorizationToken")).toEqual(["*"]);
      expect(byAction.get("ecr:PutImage")).toEqual([ref("aws_ecr_repository.server.arn")]);
      expect(byAction.get("ssm:SendCommand")).toEqual([
        ref("aws_ssm_document.deploy.arn"),
        ref("aws_instance.server.arn"),
      ]);
    });
  });
});
