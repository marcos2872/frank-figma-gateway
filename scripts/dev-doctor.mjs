#!/usr/bin/env node
/**
 * Diagnóstico de ambiente para desenvolvimento.
 * Uso: pnpm env:check (a partir de src/)
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { release } from "node:os";

const isWindows = process.platform === "win32";
const isLinux = process.platform === "linux";
const missingCommands = [];
const missingPackages = [];
const warnings = [];

function checkCommand(name, args = ["--version"]) {
  const result = spawnSync(name, args, { stdio: "ignore" });
  if (result.error || result.status !== 0) {
    missingCommands.push(name);
  }
}

function checkPkgConfig(name) {
  if (!isLinux) return;
  const result = spawnSync("pkg-config", ["--exists", name], { stdio: "ignore" });
  if (result.status !== 0) {
    missingPackages.push(name);
  }
}

console.log("frank-figma-gateway · doctor\n");
console.log(`OS: ${process.platform} ${release()}`);
console.log(`Node: ${process.version}\n`);

checkCommand("rustc");
checkCommand("cargo");
checkCommand("node");
checkCommand("pnpm");

if (isLinux) {
  checkCommand("pkg-config", ["--version"]);
  checkPkgConfig("dbus-1");
  checkPkgConfig("glib-2.0");
  checkPkgConfig("gobject-2.0");
  checkPkgConfig("cairo");
  checkPkgConfig("pango");
  checkPkgConfig("gdk-pixbuf-2.0");
  checkPkgConfig("atk");
  checkPkgConfig("gdk-3.0");
  checkPkgConfig("gtk+-3.0");
  checkPkgConfig("webkit2gtk-4.1");
  checkPkgConfig("librsvg-2.0");
  checkPkgConfig("openssl");
}

if (isWindows) {
  const webview = spawnSync(
    "reg",
    [
      "query",
      "HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
      "/v",
      "pv",
    ],
    { stdio: "ignore" },
  );
  if (webview.status !== 0) {
    warnings.push("WebView2 Runtime não detectado (instale o Evergreen Runtime).");
  }
}

if (!existsSync("node_modules")) {
  warnings.push("node_modules ausente: rode `pnpm install` em src/.");
}

if (missingCommands.length === 0 && missingPackages.length === 0) {
  console.log("✔ ferramentas obrigatórias encontradas.");
} else {
  if (missingCommands.length > 0) {
    console.log("✘ comandos ausentes:");
    for (const name of missingCommands) console.log(`  - ${name}`);
  }
  if (missingPackages.length > 0) {
    console.log("\n✘ bibliotecas de sistema ausentes (Tauri no Linux):");
    for (const name of missingPackages) console.log(`  - ${name}`);
    console.log("\nInstale conforme a distro:");
    console.log(
      "  Fedora:  sudo dnf install dbus-devel glib2-devel cairo-devel pango-devel gdk-pixbuf2-devel atk-devel gtk3-devel webkit2gtk4.1-devel librsvg2-devel openssl-devel pkgconf-pkg-config",
    );
    console.log(
      "  Ubuntu:  sudo apt install libdbus-1-dev libglib2.0-dev libcairo2-dev libpango1.0-dev libgdk-pixbuf-2.0-dev libatk1.0-dev libgtk-3-dev libwebkit2gtk-4.1-dev librsvg2-dev libssl-dev pkg-config build-essential",
    );
    console.log(
      "  Arch:    sudo pacman -S --needed dbus glib2 cairo pango gdk-pixbuf2 atk gtk3 webkit2gtk-4.1 librsvg openssl pkgconf base-devel",
    );
  }
}

if (warnings.length > 0) {
  console.log("\nAvisos:");
  for (const warning of warnings) console.log(`  - ${warning}`);
}

if (missingCommands.length > 0 || missingPackages.length > 0) {
  process.exit(1);
}

console.log("\nAmbiente pronto.");
