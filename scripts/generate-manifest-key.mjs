#!/usr/bin/env node
// Gera o par de chaves Ed25519 usado para assinar o manifesto de runtime e
// grava a chave pública em `src/src-tauri/resources/keys/manifest-keys.json`.
//
// A chave privada NUNCA pode ficar dentro do repositório: o script recusa
// caminhos internos e imprime apenas a pública.
//
// Uso:
//   node scripts/generate-manifest-key.mjs --key-id 2026-01 --key-out ~/.ffg/manifest-signing-key.txt

import { generateKeyPairSync } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..");
const defaultKeysFile = resolve(
  repoRoot,
  "src/src-tauri/resources/keys/manifest-keys.json",
);

const args = parseArgs(process.argv.slice(2));
const keyId = required(args, "key-id");
const keyOut = resolve(required(args, "key-out"));
const keysFile = resolve(args["keys-file"] ?? defaultKeysFile);
const force = args.force === "true";

if (isInside(keyOut, repoRoot)) {
  fail(
    `a chave privada não pode ficar dentro do repositório: ${keyOut}\n` +
      "use um caminho fora dele, por exemplo ~/.ffg/manifest-signing-key.txt",
  );
}

if (existsSync(keysFile) && !force && !onlyPlaceholder(keysFile)) {
  fail(
    `${keysFile} já tem uma chave ativa.\n` +
      "passe --force para substituir (a chave antiga deixa de verificar releases publicados).",
  );
}

const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const privateB64 = Buffer.from(
  privateKey.export({ format: "jwk" }).d,
  "base64url",
).toString("base64");
const publicB64 = Buffer.from(
  publicKey.export({ format: "jwk" }).x,
  "base64url",
).toString("base64");

mkdirSync(dirname(keyOut), { recursive: true });
writeFileSync(keyOut, `${privateB64}\n`, { encoding: "utf8", mode: 0o600 });

const keys = [
  {
    keyId,
    algorithm: "ed25519",
    publicKey: publicB64,
    status: "active",
    notBefore: null,
    notAfter: null,
  },
];
mkdirSync(dirname(keysFile), { recursive: true });
writeFileSync(keysFile, `${JSON.stringify(keys, null, 2)}\n`, "utf8");

console.log(`chave privada: ${keyOut}`);
console.log("  mova para o gerenciador de senhas e apague o arquivo depois de configurar o GitHub");
console.log(`chave pública: ${publicB64}`);
console.log(`keyId: ${keyId}`);
console.log(`chave pública embutida em ${relative(repoRoot, keysFile)}`);
console.log("");
console.log("próximos passos:");
console.log(`  1. gh secret set MANIFEST_SIGNING_KEY < ${keyOut}`);
console.log(`  2. gh variable set MANIFEST_KEY_ID --body "${keyId}"`);
console.log(`  3. gh variable set MANIFEST_PUBLIC_KEY --body "${publicB64}"`);
console.log("  4. recompile o app para embutir a nova chave (pnpm tauri build --no-bundle)");
console.log("  5. publique a tag (git tag v0.1.0 && git push origin v0.1.0)");

function parseArgs(argv) {
  const values = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) {
      fail(`argumento desconhecido: ${argument}`);
    }
    const name = argument.slice(2);
    if (name === "force") {
      values.force = "true";
      continue;
    }
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) {
      fail(`${argument} exige um valor`);
    }
    values[name] = value;
    index += 1;
  }
  return values;
}

function required(values, name) {
  const value = values[name];
  if (!value) {
    fail(`--${name} é obrigatório`);
  }
  return value;
}

function onlyPlaceholder(path) {
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8"));
    return (
      Array.isArray(parsed) &&
      parsed.length > 0 &&
      parsed.every((key) => key.status === "retired" || key.keyId === "dev-placeholder")
    );
  } catch {
    return false;
  }
}

function isInside(candidate, parent) {
  const relativePath = relative(parent, candidate);
  return relativePath !== "" && !relativePath.startsWith("..") && !relativePath.includes(":");
}

function fail(message) {
  console.error(`generate-manifest-key: ${message}`);
  process.exit(1);
}
