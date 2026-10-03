# Agent instructions

## Versions

Every commit with real work bumps the version in the same commit (manifest + CHANGELOG entry), so any build
traces back to a commit:
- Patch for fixes and small changes, minor for features or anything breaking before 1.0, major only when the
  owner says so. In a workspace, only the crates that changed.
- Docs-only, CI-only and no-behaviour-change refactors skip it; a burst of follow-up fixes shares one bump.
- CLIs print version, git SHA and build time in `--version`, e.g. `tool 0.4.2 (a1b2c3d-dirty, built 3 Oct 2026
  18:20)`: a small `build.rs` without extra crates (`git rev-parse --short HEAD`, `-dirty` when
  `git status --porcelain` isn't empty, `rerun-if-changed` on `.git/HEAD` and `.git/index`, `unknown` without
  git). Firmware reports the same through `fw_info`. When touching a CLI that lacks it, add it.
