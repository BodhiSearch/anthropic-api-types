import { readFileSync, writeFileSync, mkdirSync } from "node:fs";

interface OpenAPISpec {
  openapi: string;
  components: { schemas: Record<string, unknown> };
}

function rewriteRefs(obj: unknown): unknown {
  if (obj === null || obj === undefined) return obj;
  if (Array.isArray(obj)) return obj.map(rewriteRefs);
  if (typeof obj !== "object") return obj;

  const record = obj as Record<string, unknown>;
  const result: Record<string, unknown> = {};

  for (const [key, value] of Object.entries(record)) {
    if (key === "$ref" && typeof value === "string") {
      const prefix = "#/components/schemas/";
      if (value.startsWith(prefix)) {
        result[key] = `#/definitions/${value.slice(prefix.length)}`;
      } else {
        result[key] = value;
      }
    } else {
      result[key] = rewriteRefs(value);
    }
  }

  return result;
}

function stripOpenAPI31(obj: unknown): unknown {
  if (obj === null || obj === undefined) return obj;
  if (Array.isArray(obj)) return obj.map(stripOpenAPI31);
  if (typeof obj !== "object") return obj;

  const record = obj as Record<string, unknown>;
  const result: Record<string, unknown> = {};

  for (const [key, value] of Object.entries(record)) {
    if (key === "const") {
      result["enum"] = [value];
    } else if (key === "examples") {
      continue;
    } else if (key === "x-stainless-override-schema") {
      continue;
    } else {
      result[key] = stripOpenAPI31(value);
    }
  }

  if (Array.isArray(result["type"])) {
    const types = result["type"] as string[];
    if (types.length === 2 && types.includes("null")) {
      const nonNull = types.find((t) => t !== "null");
      result["type"] = nonNull;
      const existing = result["anyOf"] || result["oneOf"];
      if (!existing) {
        // nothing extra needed
      }
    } else if (types.length === 1) {
      result["type"] = types[0];
    }
  }

  return result;
}

function main() {
  const spec: OpenAPISpec = JSON.parse(
    readFileSync("generated/filtered-openapi.json", "utf-8")
  );

  const outDir = "generated/schemas";
  mkdirSync(outDir, { recursive: true });

  const allDefinitions: Record<string, unknown> = {};
  for (const [name, schema] of Object.entries(spec.components.schemas)) {
    allDefinitions[name] = stripOpenAPI31(rewriteRefs(schema));
  }

  const topLevelSchemas = [
    "CreateMessageParams",
    "Message",
    "ListResponse_ModelInfo_",
    "ModelInfo",
    "ErrorResponse",
  ];

  for (const name of topLevelSchemas) {
    if (!(name in allDefinitions)) {
      console.warn(`Warning: ${name} not found in definitions`);
      continue;
    }

    const schema = {
      $schema: "http://json-schema.org/draft-07/schema#",
      title: name,
      definitions: allDefinitions,
      ...allDefinitions[name] as Record<string, unknown>,
    };

    const outPath = `${outDir}/${name}.json`;
    writeFileSync(outPath, JSON.stringify(schema, null, 2));
    console.log(`Wrote ${outPath}`);
  }

  console.log(`\nExtracted ${topLevelSchemas.length} top-level schemas with ${Object.keys(allDefinitions).length} shared definitions`);
}

main();
