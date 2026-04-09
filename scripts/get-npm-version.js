#!/usr/bin/env node

import https from "https";

function fetchNpmVersion(packageName) {
  return new Promise((resolve) => {
    const url = `https://registry.npmjs.org/${encodeURIComponent(packageName)}/latest`;

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
              console.error(`npm registry returned status ${response.statusCode}`);
              resolve("0.0.0");
              return;
            }
            const info = JSON.parse(data);
            const version = info.version;
            if (version && /^\d+\.\d+\.\d+/.test(version)) {
              resolve(version);
            } else {
              console.error("Invalid version format from npm");
              resolve("0.0.0");
            }
          } catch (error) {
            console.error("Error parsing npm response:", error.message);
            resolve("0.0.0");
          }
        });
      }
    );

    request.on("error", (error) => {
      console.error("Error fetching from npm:", error.message);
      resolve("0.0.0");
    });

    request.setTimeout(10000, () => {
      request.destroy();
      console.error("Request timeout fetching from npm");
      resolve("0.0.0");
    });
  });
}

async function main() {
  const packageName = process.argv[2];
  if (!packageName) {
    console.error("Usage: node get-npm-version.js <package-name>");
    process.exit(1);
  }
  const version = await fetchNpmVersion(packageName);
  console.log(version);
}

main().catch((err) => {
  console.error("Error:", err.message);
  console.log("0.0.0");
  process.exit(1);
});
