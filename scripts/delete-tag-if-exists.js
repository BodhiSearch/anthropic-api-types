#!/usr/bin/env node

import { execSync } from "child_process";
import readline from "readline";

function git(command) {
  try {
    return execSync(command, { encoding: "utf8" }).trim();
  } catch {
    return null;
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

function tagExists(tagName) {
  return git(`git rev-parse "${tagName}"`) !== null;
}

async function deleteTagIfExists(tagName) {
  console.log(`Checking for existing tag ${tagName}...`);

  if (!tagExists(tagName)) {
    console.log(`  Tag ${tagName} does not exist, continuing...`);
    return;
  }

  console.log(`Warning: Tag ${tagName} already exists.`);
  const answer = await promptUser(`Delete and recreate tag ${tagName}? [y/N] `);

  if (answer === "y" || answer === "yes") {
    git(`git tag -d "${tagName}"`);
    console.log(`  Deleted local tag ${tagName}`);
    git(`git push --delete origin "${tagName}"`);
    console.log(`  Deleted remote tag ${tagName}`);
  } else {
    console.log("Aborting release.");
    process.exit(1);
  }
}

const tagName = process.argv[2];
if (!tagName) {
  console.error("Usage: node delete-tag-if-exists.js <tag-name>");
  process.exit(1);
}

deleteTagIfExists(tagName).catch((error) => {
  console.error("Error:", error.message);
  process.exit(1);
});
