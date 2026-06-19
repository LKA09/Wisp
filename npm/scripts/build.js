#!/usr/bin/env node
'use strict';

/**
 * Build helper — compiles the Rust binary and copies it into npm/dist/.
 * Run from the repo root: node npm/scripts/build.js
 * Or via: npm run build
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const repoRoot = path.join(__dirname, '..', '..');
const crateDir = path.join(repoRoot, 'wisp');
const distDir = path.join(__dirname, '..', 'dist');

const isWin = process.platform === 'win32';
const binaryName = isWin ? 'wisp.exe' : 'wisp';
const releaseBin = path.join(crateDir, 'target', 'release', binaryName);

// 1. Build
console.log('[wisp] Building Rust binary (cargo build --release)...');
try {
  execSync('cargo build --release', { cwd: crateDir, stdio: 'inherit' });
} catch (e) {
  console.error('[wisp] cargo build failed. Is Rust installed? https://rustup.rs/');
  process.exit(1);
}

// 2. Copy
fs.mkdirSync(distDir, { recursive: true });
const dest = path.join(distDir, binaryName);
fs.copyFileSync(releaseBin, dest);

if (!isWin) {
  fs.chmodSync(dest, 0o755);
}

console.log(`[wisp] Binary copied to ${path.relative(repoRoot, dest)}`);
console.log('[wisp] Done. Run `wisp` or `cd npm && npm link` to install globally.');
