#!/usr/bin/env node
// Generates the signed runtime manifest consumed by the desktop app.
// Usage:
//   node scripts/manifest.mjs --litellm dist/litellm.zip --pgsql dist/pgsql.zip \
//     --base-url https://github.com/org/repo/releases/download/v0.1.0 \
//     --out dist/manifest.json

import { createHash } from "node:crypto";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

const args = parseArgs(process.argv.slice(2));
const litellmPath = resolve(required(args, "litellm"));
const pgsqlPath = resolve(required(args, "pgsql"));
const baseUrl = required(args, "base-url").replace(/\/$/, "");
const out = resolve(args.out ?? "dist/manifest.json");
const litellmVersion = args["litellm-version"] ?? "1.108.1";
const pgsqlVersion = args["pgsql-version"] ?? "16.9";
const minAppVersion = args["min-app-version"] ?? "0.1.0";

const manifest = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  minAppVersion,
  litellm: component(
    litellmPath,
    litellmVersion,
    baseUrl,
    args["litellm-installed-size"],
    "litellm.exe",
  ),
  pgsql: component(
    pgsqlPath,
    pgsqlVersion,
    baseUrl,
    args["pgsql-installed-size"],
    "bin/postgres.exe",
  ),
};

writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
console.log(`manifesto gerado: ${out}`);
console.log(
  `litellm ${manifest.litellm.version} · ${manifest.litellm.size} bytes · ${manifest.litellm.sha256}`,
);
console.log(
  `pgsql ${manifest.pgsql.version} · ${manifest.pgsql.size} bytes · ${manifest.pgsql.sha256}`,
);

function component(path, version, base, installedSize, entrypoint) {
  const size = statSync(path).size;
  if (size === 0) {
    throw new Error(`arquivo vazio: ${path}`);
  }
  return {
    version,
    url: `${base}/${basename(path)}`,
    sha256: sha256(path),
    size,
    installedSize: installedSize ? Number(installedSize) : size,
    entrypoint,
    versionArgs: ["--version"],
  };
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function required(values, name) {
  const value = values[name];
  if (!value) {
    throw new Error(`--${name} é obrigatório`);
  }
  return value;
}

function parseArgs(argv) {
  const values = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) {
      throw new Error(`argumento desconhecido: ${argument}`);
    }
    const name = argument.slice(2);
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`${argument} exige um valor`);
    }
    values[name] = value;
    index += 1;
  }
  return values;
}
