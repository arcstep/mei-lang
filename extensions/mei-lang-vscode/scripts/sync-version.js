"use strict";

const fs = require("fs");
const path = require("path");

const cargoTomlPath = path.resolve(__dirname, "..", "..", "..", "Cargo.toml");
const packageJsonPath = path.resolve(__dirname, "..", "package.json");

const cargo = fs.readFileSync(cargoTomlPath, "utf8");
const match = cargo.match(
  /\[workspace\.package\][^\[]*?^\s*version\s*=\s*"([^"]+)"/m
);
if (!match) {
  console.error("Failed to read [workspace.package].version from Cargo.toml");
  process.exit(1);
}

const version = match[1];
const pkg = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
if (pkg.version === version) {
  console.log(`version already ${version}`);
  process.exit(0);
}

pkg.version = version;
fs.writeFileSync(packageJsonPath, `${JSON.stringify(pkg, null, 2)}\n`);
console.log(`synced package.json version -> ${version}`);
