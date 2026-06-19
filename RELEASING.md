# Releasing Wisp

## Release checklist

1. **Bump versions** — update both in sync:
   - `wisp/Cargo.toml` → `version = "X.Y.Z"`
   - `npm/package.json` → `"version": "X.Y.Z"`

2. **Build release binaries** for each target:
   ```bash
   # Windows (x86_64)
   cargo build --release
   cp target/release/wisp.exe ../npm/dist/wisp.exe   # for local testing

   # Linux (x86_64) — cross-compile or build on Linux CI
   cargo build --release --target x86_64-unknown-linux-musl

   # macOS (arm64) — build on Apple Silicon or use cross
   cargo build --release --target aarch64-apple-darwin
   ```

3. **Name release assets** using this convention:
   ```
   wisp-windows-x86_64.exe
   wisp-linux-x86_64
   wisp-darwin-aarch64
   wisp-darwin-x86_64
   ```

4. **Create a GitHub Release** tagged `vX.Y.Z` and upload the binaries above.
   The `npm/scripts/postinstall.js` downloads assets from this exact URL pattern:
   ```
   https://github.com/LKA09/Wisp/releases/download/vX.Y.Z/<asset-name>
   ```

5. **Publish to npm:**
   ```bash
   cd npm
   npm publish --access public
   ```

## TODO: Automate with GitHub Actions

A release workflow (`.github/workflows/release.yml`) should:
- Trigger on `push: tags: ['v*']`
- Build all four targets via a matrix job (Linux musl cross-compile, Windows, macOS arm64/x64)
- Upload artifacts to the GitHub Release
- Run `npm publish` after all binaries are attached

This is not yet implemented. Until then, releases are manual.
