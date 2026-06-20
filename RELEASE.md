# Release Process

## Overview

Releases are fully automated via `.github/workflows/release.yml`. The workflow builds six platform binaries, then creates or updates a GitHub Release with all assets.

## Triggering a release

### Option 1 — Push a tag (recommended)

```sh
git tag v0.2.0
git push origin v0.2.0
```

The workflow triggers automatically on any `v*` tag push. The release name and notes are generated from the tag.

### Option 2 — Manual dispatch

Go to **Actions → Release → Run workflow** and enter the tag name (e.g. `v0.2.0`). The tag must already exist or the `gh release create` step will reference a non-existent ref.

## Build matrix

| Job | Runner | Output asset |
|---|---|---|
| `build-linux-x86_64` | `ubuntu-latest` | `wisp-linux-x86_64` |
| `build-linux-aarch64` | `ubuntu-latest` (cross) | `wisp-linux-aarch64` |
| `build-windows-x86_64` | `windows-latest` | `wisp-windows-x86_64.exe` |
| `build-windows-aarch64` | `windows-11-arm` | `wisp-windows-aarch64.exe` |
| `build-macos-x86_64` | `macos-13` (Intel) | `wisp-darwin-x86_64` |
| `build-macos-aarch64` | `macos-latest` (Apple Silicon) | `wisp-darwin-aarch64` |

All six artifacts are uploaded to the GitHub Release. If the release already exists, assets are replaced with `--clobber`.

## Version bump checklist

1. Update `version` in `wisp/Cargo.toml`.
2. Update `version` in `npm/package.json` to match.
3. Commit: `git commit -am "chore: bump version to v0.2.0"`
4. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`
5. Wait for the release workflow to complete (~10 min).
6. Verify all six assets appear on the GitHub Releases page.
7. Publish to npm: `cd npm && npm publish --access public`

> **Order matters:** publish to npm only after the GitHub Release assets are live, because the npm postinstall script downloads from the release.

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
