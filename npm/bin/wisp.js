#!/usr/bin/env node
'use strict';

const path = require('path');
const { spawnSync } = require('child_process');

const platform = process.platform;
const binaryName = platform === 'win32' ? 'wisp.exe' : 'wisp';
const binaryPath = path.join(__dirname, '..', 'dist', binaryName);

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',
  env: process.env,
});

if (result.error) {
  if (result.error.code === 'ENOENT') {
    console.error('Error: Wisp binary not found at ' + binaryPath);
    console.error('');
    console.error('Build and install the binary:');
    console.error('  cd wisp');
    console.error('  cargo build --release');
    console.error('  cp target/release/wisp ../npm/dist/       # Linux/macOS');
    console.error('  copy target\\release\\wisp.exe ..\\npm\\dist\\  # Windows');
    process.exit(1);
  }
  console.error('Error: ' + result.error.message);
  process.exit(1);
}

process.exit(result.status !== null ? result.status : 0);
