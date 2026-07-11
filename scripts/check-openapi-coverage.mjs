import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const controllersRoot = path.join(root, "apps", "server", "src");
const openapiPath = path.join(root, "packages", "contracts", "openapi.yaml");

function walkControllers(directory, result = []) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const resolved = path.join(directory, entry.name);
    if (entry.isDirectory()) walkControllers(resolved, result);
    else if (entry.name.endsWith(".controller.ts")) result.push(resolved);
  }
  return result;
}

function normalizeRoute(value) {
  const normalized = `/${value}`
    .replaceAll("\\", "/")
    .replace(/:\w+/g, "{}")
    .replace(/\{[^}]+\}/g, "{}")
    .replace(/\/{2,}/g, "/")
    .replace(/\/$/, "");
  return normalized || "/";
}

function sourceRoutes() {
  const routes = new Set();
  for (const file of walkControllers(controllersRoot)) {
    const source = fs.readFileSync(file, "utf8");
    const controller = source.match(/@Controller\((?:'([^']*)')?\)/);
    if (!controller) continue;
    const prefix = controller[1] ?? "";
    for (const match of source.matchAll(
      /@(Get|Post|Patch|Delete)\((?:'([^']*)')?\)/g,
    )) {
      const route = [prefix, match[2] ?? ""].filter(Boolean).join("/");
      routes.add(`${match[1].toUpperCase()} ${normalizeRoute(route)}`);
    }
  }
  return routes;
}

function openapiRoutes(source) {
  const routes = new Set();
  let inPaths = false;
  let currentPath = null;
  for (const line of source.split(/\r?\n/)) {
    if (line === "paths:") {
      inPaths = true;
      continue;
    }
    if (line === "components:") break;
    if (!inPaths) continue;
    const pathMatch = line.match(/^  (\/[^:]+):\s*$/);
    if (pathMatch) {
      currentPath = pathMatch[1];
      continue;
    }
    const methodMatch = line.match(/^    (get|post|patch|delete):\s*$/);
    if (currentPath && methodMatch) {
      routes.add(
        `${methodMatch[1].toUpperCase()} ${normalizeRoute(currentPath)}`,
      );
    }
  }
  return routes;
}

function assertSchemaReferences(source) {
  const schemas = new Set();
  let inSchemas = false;
  for (const line of source.split(/\r?\n/)) {
    if (line === "  schemas:") {
      inSchemas = true;
      continue;
    }
    if (!inSchemas) continue;
    const match = line.match(/^    ([A-Za-z0-9_-]+):\s*$/);
    if (match) schemas.add(match[1]);
  }
  const references = [
    ...source.matchAll(/#\/components\/schemas\/([A-Za-z0-9_-]+)/g),
  ].map((match) => match[1]);
  const missing = [...new Set(references)].filter((name) => !schemas.has(name));
  if (missing.length) {
    throw new Error(`Missing OpenAPI schema references: ${missing.join(", ")}`);
  }
}

const openapiSource = fs.readFileSync(openapiPath, "utf8");
const actual = sourceRoutes();
const declared = openapiRoutes(openapiSource);
const missing = [...actual].filter((route) => !declared.has(route)).sort();
const extra = [...declared].filter((route) => !actual.has(route)).sort();

assertSchemaReferences(openapiSource);
if (missing.length || extra.length) {
  if (missing.length)
    process.stderr.write(`Missing routes:\n${missing.join("\n")}\n`);
  if (extra.length)
    process.stderr.write(`Unknown routes:\n${extra.join("\n")}\n`);
  process.exitCode = 1;
} else {
  process.stdout.write(
    `OpenAPI route coverage OK: ${actual.size} controller methods.\n`,
  );
}
