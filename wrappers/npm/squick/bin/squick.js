#!/usr/bin/env node
// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0
//
// Locates the platform-specific Squick binary installed via
// optionalDependencies and execs it with the caller's arguments. Stays
// transparent to stdio so MCP and other consumers see no wrapping layer.

"use strict";

const { spawnSync } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");

const PLATFORM = `${process.platform}-${process.arch}`;

const SUPPORTED = new Map([
  ["linux-x64", "@hubhorizonllc/squick-linux-x64"],
  ["linux-arm64", "@hubhorizonllc/squick-linux-arm64"],
  ["darwin-x64", "@hubhorizonllc/squick-darwin-x64"],
  ["darwin-arm64", "@hubhorizonllc/squick-darwin-arm64"],
  ["win32-x64", "@hubhorizonllc/squick-win32-x64"],
]);

const pkgName = SUPPORTED.get(PLATFORM);
if (!pkgName) {
  console.error(
    `squick: unsupported platform ${PLATFORM}. Supported: ${[...SUPPORTED.keys()].join(", ")}.\n` +
      "Install the binary manually from https://github.com/horizon-llc/squick/releases.",
  );
  process.exit(1);
}

const binaryName = process.platform === "win32" ? "squick.exe" : "squick";

let binaryPath;
try {
  binaryPath = require.resolve(`${pkgName}/bin/${binaryName}`);
} catch (err) {
  console.error(
    `squick: missing optional dependency ${pkgName}.\n` +
      "If npm was invoked with --no-optional or in a constrained environment,\n" +
      `install the platform package directly:  npm i -g ${pkgName}@1.0.0`,
  );
  process.exit(1);
}

if (!fs.existsSync(binaryPath)) {
  console.error(`squick: binary not found at ${binaryPath}`);
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: true,
});

if (result.error) {
  console.error(`squick: failed to spawn binary: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 0);
