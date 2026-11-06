#!/usr/bin/env node
// Garante que a chave pública embutida no app é a mesma que assina o release.
//
// Sem isso é possível publicar um manifesto assinado por uma chave que o app
// instalado não conhece — o download falha em toda máquina já instalada.
//
// Uso (CI):
//   MANIFEST_KEY_ID=2026-01 MANIFEST_PUBLIC_KEY=<base64> node scripts/check-manifest-key.mjs

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const keysFile = resolve(
  process.env.MANIFEST_KEYS_FILE ??
    resolve(repoRoot, "src/src-tauri/resources/keys/manifest-keys.json"),
);

const keyId = (process.env.MANIFEST_KEY_ID ?? "").trim();
const publicKey = (process.env.MANIFEST_PUBLIC_KEY ?? "").trim();

if (!keyId || !publicKey) {
  fail("MANIFEST_KEY_ID e MANIFEST_PUBLIC_KEY precisam estar definidos");
}

const keys = readKeys();
const active = keys.filter((key) => key.status === "active");

if (active.length === 0) {
  fail(
    `${keysFile} não tem nenhuma chave com status "active".\n` +
      "rode: node scripts/generate-manifest-key.mjs --key-id <id> --key-out <caminho fora do repo>",
  );
}

const match = active.find((key) => key.keyId === keyId);
if (!match) {
  fail(
    `o app embute as chaves [${active.map((key) => key.keyId).join(", ")}], ` +
      `mas o release assina com "${keyId}".`,
  );
}

if (match.publicKey.trim() !== publicKey) {
  fail(
    `a chave pública de "${keyId}" no app não confere com MANIFEST_PUBLIC_KEY.\n` +
      "regenere o par e recompile o app antes de publicar.",
  );
}

if (match.algorithm !== "ed25519") {
  fail(`algoritmo inesperado para "${keyId}": ${match.algorithm}`);
}

console.log(`chave ok: ${keyId} (ed25519, ativa, embutida no app)`);

function readKeys() {
  let raw;
  try {
    raw = readFileSync(keysFile, "utf8");
  } catch (error) {
    fail(`não foi possível ler ${keysFile}: ${error.message}`);
  }
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      fail(`${keysFile} deveria conter uma lista de chaves`);
    }
    return parsed;
  } catch (error) {
    fail(`${keysFile} não é JSON válido: ${error.message}`);
  }
}

function fail(message) {
  console.error(`check-manifest-key: ${message}`);
  process.exit(1);
}
