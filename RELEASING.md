# Releasing Wisp

> **See [`RELEASE.md`](RELEASE.md) for the full, up-to-date release guide.**

## Quick reference

The release workflow (`.github/workflows/release.yml`) is fully automated:
- Triggers on `push: tags: ['v*']` or via **Actions → Release → Run workflow**.
- Builds all six platform binaries and uploads them to a GitHub Release.
- If the release already exists, assets are replaced (`--clobber`).

## Expected release asset names

All six must be present on the GitHub Release before running `npm publish`:

```
wisp-windows-x86_64.exe
wisp-windows-aarch64.exe
wisp-linux-x86_64
wisp-linux-aarch64
wisp-darwin-x86_64
wisp-darwin-aarch64
```

These names are hard-coded in `npm/scripts/postinstall.js` and `npm/bin/wisp.js`.

## Publish order

1. Bump `version` in `wisp/Cargo.toml` and `npm/package.json` (must match).
2. Run local validation: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm pack --dry-run`.
3. Commit, tag, and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
4. Wait for the Release workflow to finish and confirm all six assets are live.
5. `cd npm && npm publish --access public`

> The npm postinstall download is non-fatal — if assets are missing at install time,
> users see a warning and must build from source. Publish to npm only after assets are confirmed.
