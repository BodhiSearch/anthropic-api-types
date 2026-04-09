#!/usr/bin/env node

function incrementPatchVersion(version) {
  const match = version.match(/^(\d+)\.(\d+)\.(\d+)$/);
  if (!match) {
    throw new Error(`Invalid version format: ${version}. Expected x.y.z`);
  }
  const [, major, minor, patch] = match;
  return `${major}.${minor}.${parseInt(patch, 10) + 1}`;
}

function main() {
  const currentVersion = process.argv[2];
  if (!currentVersion) {
    console.error("Usage: node increment-version.js <version>");
    process.exit(1);
  }
  try {
    console.log(incrementPatchVersion(currentVersion));
  } catch (error) {
    console.error("Error:", error.message);
    process.exit(1);
  }
}

main();
