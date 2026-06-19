#!/usr/bin/env node
'use strict';

/**
 * Wisp postinstall — downloads the platform-appropriate binary from GitHub Releases.
 * Skips silently if the binary already exists (local development / manual build).
 * Never hard-fails npm install; only warns on download errors.
 */

const https = require('https');
const fs = require('fs');
const path = require('path');

const pkg = require('../package.json');
const version = pkg.version;
const distDir = path.join(__dirname, '..', 'dist');
const binaryName = process.platform === 'win32' ? 'wisp.exe' : 'wisp';
const binaryPath = path.join(distDir, binaryName);

// Already installed (local dev or previous run) — skip.
if (fs.existsSync(binaryPath)) {
  console.log('[wisp] Binary already present, skipping download.');
  process.exit(0);
}

// Map Node platform/arch to Rust target triples used in release asset names.
const PLATFORM_MAP = {
  darwin:  'darwin',
  linux:   'linux',
  win32:   'windows',
};
const ARCH_MAP = {
  x64:   'x86_64',
  arm64: 'aarch64',
};

const plat = PLATFORM_MAP[process.platform];
const arch = ARCH_MAP[process.arch];

if (!plat || !arch) {
  warn(`Unsupported platform: ${process.platform}-${process.arch}`);
  process.exit(0);
}

const ext = process.platform === 'win32' ? '.exe' : '';
const assetName = `wisp-${plat}-${arch}${ext}`;
const repoOwner = 'LKA09';
const repoName = 'Wisp';
const downloadUrl =
  `https://github.com/${repoOwner}/${repoName}/releases/download/v${version}/${assetName}`;

console.log(`[wisp] Downloading v${version} for ${process.platform}-${process.arch}...`);
console.log(`[wisp] ${downloadUrl}`);

fs.mkdirSync(distDir, { recursive: true });

get(downloadUrl, (err, data) => {
  if (err) {
    warn(`Download failed: ${err.message}`);
    warn('Build from source instead:');
    warn('  cd wisp && cargo build --release');
    warn('  node npm/scripts/build.js');
    process.exit(0);
  }

  fs.writeFileSync(binaryPath, data);

  if (process.platform !== 'win32') {
    fs.chmodSync(binaryPath, 0o755);
  }

  console.log('[wisp] Installed successfully.');
});

// ─── helpers ──────────────────────────────────────────────────────────────────

function warn(msg) {
  console.warn('[wisp] ' + msg);
}

/** HTTP GET with redirect following (GitHub releases redirect to S3). */
function get(url, callback, redirects = 0) {
  if (redirects > 5) {
    callback(new Error('Too many redirects'));
    return;
  }

  https.get(url, { headers: { 'User-Agent': 'wisp-postinstall' } }, (res) => {
    if (res.statusCode === 301 || res.statusCode === 302) {
      get(res.headers.location, callback, redirects + 1);
      return;
    }
    if (res.statusCode !== 200) {
      callback(new Error(`HTTP ${res.statusCode} from ${url}`));
      return;
    }

    const chunks = [];
    res.on('data', (c) => chunks.push(c));
    res.on('end', () => callback(null, Buffer.concat(chunks)));
    res.on('error', callback);
  }).on('error', callback);
}
