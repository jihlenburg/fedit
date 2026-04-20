# fedit

A tiny, real text editor built on Tauri 2 — the teaching project for the
interactive Rust tutorial at [jihlenburg/rustic](https://github.com/jihlenburg/rustic).
Each git tag below snapshots a chapter's state so readers can `git checkout`
forward and backward through the build.

## Run it

```
npm install
npm run tauri dev
```

## Build a binary

```
npm run tauri build
```

Installers land in `src-tauri/target/release/bundle/` — `.dmg` on macOS,
`.msi` + `.exe` on Windows, `.deb` / `.rpm` / `AppImage` on Linux.

## Chapter tags

Thirteen per-chapter tags — chapters that don't change Rust or JS code
(frontend-only tweaks, checkpoints, outro) are intentionally skipped.

```
git tag -l 'ch*'
git checkout ch05    # "read a hardcoded file" chapter state
git checkout main    # back to the latest
```

`main` tracks the most recent shipping chapter.

## Contributing / first-clone setup

This repo ships a tracked commit-msg hook that enforces the attribution
policy (see `.githooks/commit-msg`). Point your local Git at it once,
after cloning:

```
git config core.hooksPath .githooks
```

## Licence

MIT — see [LICENSE](LICENSE).
