import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { parse as parseYaml } from "yaml";
import { downloadAndFilter } from "./filter-spec.js";

const STATS_URL =
  "https://raw.githubusercontent.com/anthropics/anthropic-sdk-typescript/refs/heads/main/.stats.yml";

const HASHES_PATH = "generated/hashes.json";

interface Hashes {
  openapi_spec_hash: string;
  anthropic_types_hash: string;
}

interface StatsYml {
  openapi_spec_url: string;
  openapi_spec_hash: string;
}

function md5(content: string): string {
  return createHash("md5").update(content).digest("hex");
}

function readHashes(): Hashes {
  if (!existsSync(HASHES_PATH)) {
    return { openapi_spec_hash: "", anthropic_types_hash: "" };
  }
  return JSON.parse(readFileSync(HASHES_PATH, "utf-8"));
}

function writeHashes(hashes: Hashes): void {
  mkdirSync("generated", { recursive: true });
  writeFileSync(HASHES_PATH, JSON.stringify(hashes, null, 2) + "\n");
}

async function fetchStats(): Promise<StatsYml> {
  console.log("Fetching upstream .stats.yml...");
  const response = await fetch(STATS_URL);
  if (!response.ok) {
    throw new Error(`Failed to fetch .stats.yml: ${response.status} ${response.statusText}`);
  }
  const text = await response.text();
  const parsed = parseYaml(text) as Record<string, string>;

  if (!parsed.openapi_spec_url || !parsed.openapi_spec_hash) {
    throw new Error("Missing openapi_spec_url or openapi_spec_hash in .stats.yml");
  }

  return {
    openapi_spec_url: parsed.openapi_spec_url,
    openapi_spec_hash: parsed.openapi_spec_hash,
  };
}

async function main() {
  const force = !!process.env.FORCE;
  const stats = await fetchStats();
  const current = readHashes();

  console.log(`Upstream spec hash: ${stats.openapi_spec_hash}`);
  console.log(`Stored spec hash:   ${current.openapi_spec_hash || "(none)"}`);

  if (!force && stats.openapi_spec_hash === current.openapi_spec_hash) {
    console.log("\nNo upstream spec changes detected.");
    process.exit(0);
  }

  if (force) {
    console.log("\nForce mode: skipping hash comparison.");
  } else {
    console.log("\nUpstream spec hash changed, downloading and filtering...");
  }

  const filteredJson = await downloadAndFilter(stats.openapi_spec_url);
  const newTypesHash = md5(filteredJson);

  console.log(`\nFiltered types hash: ${newTypesHash}`);
  console.log(`Stored types hash:   ${current.anthropic_types_hash || "(none)"}`);

  if (!force && newTypesHash === current.anthropic_types_hash) {
    console.log("\nFiltered types unchanged (only non-target endpoints changed upstream).");
    writeHashes({
      openapi_spec_hash: stats.openapi_spec_hash,
      anthropic_types_hash: current.anthropic_types_hash,
    });
    console.log("Updated openapi_spec_hash only.");
    process.exit(0);
  }

  writeHashes({
    openapi_spec_hash: stats.openapi_spec_hash,
    anthropic_types_hash: newTypesHash,
  });
  console.log("\nHashes updated. Types need regeneration.");
  process.exit(2);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
