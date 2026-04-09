#!/usr/bin/env node

import { execSync } from "child_process";
import readline from "readline";

function git(command) {
  try {
    return execSync(command, { encoding: "utf8" }).trim();
  } catch (error) {
    console.error(`Error executing: ${command}`);
    console.error(error.message);
    process.exit(1);
  }
}

function promptUser(question) {
  if (process.env.CONFIRM === "y") {
    console.log(`${question}(auto-confirmed)`);
    return Promise.resolve("y");
  }
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  return new Promise((resolve) => {
    rl.question(question, (answer) => {
      rl.close();
      resolve(answer.toLowerCase());
    });
  });
}

async function checkGitSync() {
  console.log("Checking git repository status...");

  const status = git("git status --porcelain");
  if (status) {
    console.log("Warning: You have uncommitted changes:");
    console.log(status);
    const answer = await promptUser("Continue with uncommitted changes? [y/N] ");
    if (answer !== "y" && answer !== "yes") {
      console.log("Aborting release.");
      process.exit(1);
    }
  } else {
    console.log("  No uncommitted changes");
  }

  console.log("Fetching latest from remote...");
  try {
    git("git fetch origin main");
  } catch {
    console.log("Warning: Could not fetch from origin/main");
    return;
  }

  const localHead = git("git rev-parse HEAD");
  let remoteHead;
  try {
    remoteHead = git("git rev-parse origin/main");
  } catch {
    console.log("Warning: Could not resolve origin/main");
    return;
  }

  if (localHead !== remoteHead) {
    console.log("Warning: Local branch differs from origin/main");
    console.log(`  Local:  ${localHead}`);
    console.log(`  Remote: ${remoteHead}`);

    const unpushed = git("git log origin/main..HEAD --oneline");
    if (unpushed) {
      console.log("Unpushed commits:");
      console.log(unpushed);
    }

    const unpulled = git("git log HEAD..origin/main --oneline");
    if (unpulled) {
      console.log("Unpulled commits:");
      console.log(unpulled);
    }

    const answer = await promptUser("Continue anyway? Tag will be placed on local HEAD. [y/N] ");
    if (answer !== "y" && answer !== "yes") {
      console.log("Aborting release.");
      process.exit(1);
    }
  } else {
    console.log("  Local and remote are in sync");
  }

  console.log("  Git repository ready for release");
}

checkGitSync().catch((error) => {
  console.error("Error:", error.message);
  process.exit(1);
});
