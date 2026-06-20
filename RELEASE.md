# Release Process

## Overview

Releases are fully automated via `.github/workflows/release.yml`. The workflow builds six platform binaries, then creates or updates a GitHub Release with all assets.

## Triggering a release

### Option 1 — Push a tag (recommended)

```sh
git tag v0.1.0
git push origin v0.1.0
```

The workflow triggers automatically on any `v*` tag push. The release title and changelog notes are generated automatically from the tag.

### Option 2 — Manual dispatch

Go to **Actions → Release → Run workflow** on GitHub and enter the tag name (e.g. `v0.1.0`). Create the tag locally and push it first, or the `gh release create` step will reference a non-existent ref.

## Build matrix

| Job | Runner | Output asset |
|---|---|---|
| `build-linux-x86_64` | `ubuntu-latest` | `wisp-linux-x86_64` |
| `build-linux-aarch64` | `ubuntu-latest` (cross-compile to aarch64-unknown-linux-gnu) | `wisp-linux-aarch64` |
| `build-windows-x86_64` | `windows-latest` | `wisp-windows-x86_64.exe` |
| `build-windows-aarch64` | `windows-11-arm` (GitHub-hosted ARM64, GA for public repos) | `wisp-windows-aarch64.exe` |
| `build-macos-x86_64` | `macos-13` (Intel) | `wisp-darwin-x86_64` |
| `build-macos-aarch64` | `macos-14` (Apple Silicon M1) | `wisp-darwin-aarch64` |

All six artifacts are uploaded to the GitHub Release. If the release already exists, assets are replaced with `--clobber`.

### Runner notes

- **`windows-11-arm`** — GitHub-hosted Windows ARM64 runner, generally available for public repositories. Native compile; no cross-toolchain needed.
- **`macos-14`** — pinned explicitly to Apple Silicon (M1). `macos-latest` is intentionally avoided because its target arch changes when GitHub updates the default.
- **`macos-13`** — last Intel-based macOS runner; used for the x86_64 build.

## Version bump checklist

1. Update `version` in `wisp/Cargo.toml`.
2. Update `version` in `npm/package.json` to the same value.
3. Run local validation (see below).
4. Commit: `git commit -am "chore: bump version to vX.Y.Z"`
5. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`
6. Wait for the Release workflow to finish (~10 min).
7. Confirm all six assets appear on the GitHub Releases page with exact names:
   - `wisp-linux-x86_64`, `wisp-linux-aarch64`
   - `wisp-windows-x86_64.exe`, `wisp-windows-aarch64.exe`
   - `wisp-darwin-x86_64`, `wisp-darwin-aarch64`
8. Run `npm pack --dry-run` from the `npm/` directory — verify the file list looks right.
9. Publish to npm: `cd npm && npm publish --access public`

> **Order matters:** publish to npm only after the GitHub Release assets are live, because the npm postinstall script downloads from the release.

## Local validation (step 3 above)

```sh
cargo fmt --check --manifest-path wisp/Cargo.toml
cargo clippy --manifest-path wisp/Cargo.toml -- -D warnings
cargo test --manifest-path wisp/Cargo.toml
npm pack --dry-run --prefix npm
```

All four must pass before tagging.

## Re-running a failed release

If one build job fails, fix the issue and re-run the entire workflow. The `publish-release` job uses `--clobber` so re-uploading assets is safe.

## Building from source (fallback)

If the npm postinstall download fails, users can build the binary locally:

**Windows (PowerShell)**

```powershell
git clone https://github.com/LKA09/Wisp
cd Wisp\wisp
cargo build --release
New-Item -ItemType Directory -Force -Path ..\npm\dist | Out-Null
Copy-Item target\release\wisp.exe ..\npm\dist\wisp.exe
cd ..\npm
npm link
```

**Linux / macOS**

```sh
git clone https://github.com/LKA09/Wisp
cd Wisp/wisp
cargo build --release
mkdir -p ../npm/dist
cp target/release/wisp ../npm/dist/wisp
cd ../npm
npm link
```
