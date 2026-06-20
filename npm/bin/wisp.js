#!/usr/bin/env node
'use strict';

const path = require('path');
const { spawnSync } = require('child_process');

const platform = process.platform;
const arch = process.arch;

// Map platform/arch to the release asset name used in GitHub Releases.
const ASSET_NAMES = {
  win32: { x64: 'wisp-windows-x86_64.exe', arm64: 'wisp-windows-aarch64.exe' },
  linux: { x64: 'wisp-linux-x86_64',       arm64: 'wisp-linux-aarch64' },
  darwin: { x64: 'wisp-darwin-x86_64',     arm64: 'wisp-darwin-aarch64' },
};

const binaryName = platform === 'win32' ? 'wisp.exe' : 'wisp';
const binaryPath = path.join(__dirname, '..', 'dist', binaryName);

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',
  env: process.env,
});

if (result.error) {
  if (result.error.code === 'ENOENT') {
    const assetName = (ASSET_NAMES[platform] || {})[arch] || '<asset-name>';
    const pkg = require('../package.json');
    const version = pkg.version;

    console.error('Error: Wisp binary not found.');
    console.error('');
    console.error('  Expected path: ' + binaryPath);
    console.error('');
    console.error('The GitHub Release asset for v' + version + ' may be missing for this platform.');
    console.error('Expected release asset: ' + assetName);
    console.error('');
    console.error('Build from source (Windows PowerShell):');
    console.error('  cd wisp');
    console.error('  cargo build --release');
    console.error('  New-Item -ItemType Directory -Force -Path ..\\npm\\dist | Out-Null');
    console.error('  Copy-Item target\\release\\wisp.exe ..\\npm\\dist\\wisp.exe');
    console.error('');
    console.error('Build from source (Linux / macOS):');
    console.error('  cd wisp && cargo build --release');
    console.error('  mkdir -p ../npm/dist');
    console.error('  cp target/release/wisp ../npm/dist/wisp');
    console.error('');
    console.error('Expected release asset names:');
    console.error('  wisp-windows-x86_64.exe   wisp-windows-aarch64.exe');
    console.error('  wisp-linux-x86_64         wisp-linux-aarch64');
    console.error('  wisp-darwin-x86_64        wisp-darwin-aarch64');
    process.exit(1);
  }
  console.error('Error: ' + result.error.message);
  process.exit(1);
}

process.exit(result.status !== null ? result.status : 0);
