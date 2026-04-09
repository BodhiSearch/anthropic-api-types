#!/usr/bin/env node

import https from "https";

function fetchCrateVersion(crateName) {
  return new Promise((resolve) => {
    const url = `https://crates.io/api/v1/crates/${encodeURIComponent(crateName)}`;

    const request = https.get(
      url,
      { headers: { "User-Agent": "anthropic-types release script" } },
      (response) => {
        let data = "";
        response.on("data", (chunk) => { data += chunk; });
        response.on("end", () => {
          try {
            if (response.statusCode === 404) {
              resolve("0.0.0");
              return;
            }
            if (response.statusCode !== 200) {
              console.error(`crates.io returned status ${response.statusCode}`);
              resolve("0.0.0");
              return;
            }
            const info = JSON.parse(data);
            const version = info.crate?.max_stable_version || info.crate?.max_version;
            if (version && /^\d+\.\d+\.\d+/.test(version)) {
              resolve(version);
            } else {
              console.error("Invalid version format from crates.io");
              resolve("0.0.0");
            }
          } catch (error) {
            console.error("Error parsing crates.io response:", error.message);
            resolve("0.0.0");
          }
        });
      }
    );

    request.on("error", (error) => {
      console.error("Error fetching from crates.io:", error.message);
      resolve("0.0.0");
    });

    request.setTimeout(10000, () => {
      request.destroy();
      console.error("Request timeout fetching from crates.io");
      resolve("0.0.0");
    });
  });
}

async function main() {
  const crateName = process.argv[2];
  if (!crateName) {
    console.error("Usage: node get-crate-version.js <crate-name>");
    process.exit(1);
  }
  const version = await fetchCrateVersion(crateName);
  console.log(version);
}

main().catch((err) => {
  console.error("Error:", err.message);
  console.log("0.0.0");
  process.exit(1);
});
