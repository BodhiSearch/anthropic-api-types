set dotenv-load := false

export CONFIRM := env("CONFIRM", "")

# Show available targets
default:
  @just --list

# Install dependencies
setup:
  npm ci

# Check upstream Anthropic spec for changes and regenerate if needed
sync *flags:
  #!/usr/bin/env bash
  set -uo pipefail
  FORCE=""
  for arg in {{flags}}; do
    case "$arg" in
      --force|-f) FORCE="1" ;;
    esac
  done
  if [ -n "$FORCE" ]; then
    export FORCE=1
  fi
  npx tsx scripts/sync.ts
  EXIT_CODE=$?
  if [ $EXIT_CODE -eq 2 ]; then
    echo ""
    echo "Types need regeneration, running generate..."
    just generate
  elif [ $EXIT_CODE -eq 0 ]; then
    echo "No regeneration needed."
  else
    echo "Sync failed with exit code $EXIT_CODE"
    exit $EXIT_CODE
  fi

# Download and filter the OpenAPI spec (standalone, no hash checks)
filter:
  npx tsx scripts/filter-spec.ts

# Generate both TypeScript and Rust types
generate: generate-ts generate-rust

# Generate TypeScript types from filtered OpenAPI spec
# Produces both:
#   - typescript/src/openapi-typescript/openapi-schema.ts (paths/components, MSW-friendly)
#   - typescript/src/types/                                (hey-api flat type exports)
generate-ts:
  cd typescript && npm run generate

# Generate Rust types from filtered OpenAPI spec
generate-rust:
  #!/usr/bin/env bash
  set -euo pipefail
  npx tsx scripts/extract-schemas.ts
  SCHEMAS_DIR="generated/schemas"
  for schema in CreateMessageParams Message ListResponse_ModelInfo_ ModelInfo ErrorResponse; do
    echo "  Generating $schema..."
    quicktype -s schema -l rs -t "$schema" -o "${SCHEMAS_DIR}/${schema}.rs" "${SCHEMAS_DIR}/${schema}.json" 2>&1 || true
  done
  npx tsx scripts/merge-rust.ts
  cargo run --manifest-path scripts/add-utoipa-annotations/Cargo.toml --quiet
  cargo fmt --manifest-path rust/Cargo.toml

# Type-check TypeScript and Rust
check:
  cd typescript && npx tsc --noEmit
  cd rust && cargo check

# Release both TypeScript (npm) and Rust (crates.io) via tags
release *flags:
  #!/usr/bin/env bash
  set -euo pipefail
  for arg in {{flags}}; do
    case "$arg" in
      -y|--yes) export CONFIRM="y" ;;
    esac
  done
  just release-ts
  just release-rust

# Release TypeScript package via tag push
release-ts *flags:
  #!/usr/bin/env bash
  set -euo pipefail
  for arg in {{flags}}; do
    case "$arg" in
      -y|--yes) export CONFIRM="y" ;;
    esac
  done
  echo "Preparing TypeScript release..."
  node scripts/git-check-sync.js
  CURRENT=$(node scripts/get-npm-version.js @bodhiapp/anthropic-api-types)
  NEXT=$(node scripts/increment-version.js "$CURRENT")
  echo "Current npm version: $CURRENT"
  echo "Next version:        $NEXT"
  TAG_NAME="release-ts/v$NEXT"
  if [ "$CONFIRM" != "y" ]; then
    read -rp "Release TypeScript v$NEXT? [y/N] " answer
    if [ "$answer" != "y" ] && [ "$answer" != "Y" ]; then
      echo "Skipping TypeScript release."
      exit 0
    fi
  fi
  node scripts/delete-tag-if-exists.js "$TAG_NAME"
  git tag "$TAG_NAME"
  git push origin "$TAG_NAME"
  echo "Tag $TAG_NAME pushed. GitHub workflow will handle publishing."

# Release Rust crate via tag push
release-rust *flags:
  #!/usr/bin/env bash
  set -euo pipefail
  for arg in {{flags}}; do
    case "$arg" in
      -y|--yes) export CONFIRM="y" ;;
    esac
  done
  echo "Preparing Rust release..."
  node scripts/git-check-sync.js
  CURRENT=$(node scripts/get-crate-version.js anthropic-api-types)
  NEXT=$(node scripts/increment-version.js "$CURRENT")
  echo "Current crates.io version: $CURRENT"
  echo "Next version:              $NEXT"
  TAG_NAME="release-rust/v$NEXT"
  if [ "$CONFIRM" != "y" ]; then
    read -rp "Release Rust v$NEXT? [y/N] " answer
    if [ "$answer" != "y" ] && [ "$answer" != "Y" ]; then
      echo "Skipping Rust release."
      exit 0
    fi
  fi
  node scripts/delete-tag-if-exists.js "$TAG_NAME"
  git tag "$TAG_NAME"
  git push origin "$TAG_NAME"
  echo "Tag $TAG_NAME pushed. GitHub workflow will handle publishing."
