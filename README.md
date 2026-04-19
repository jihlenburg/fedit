# fedit

A tiny, real text editor built on Tauri 2. This is the companion project to the
interactive Rust tutorial at [`../rust-tutorial.html`](../rust-tutorial.html).

## Run it

```
npm install
npm run tauri dev
```

## Build a binary

```
npm run tauri build
```

## Chapter tags

Each tagged chapter (`ch01`, `ch03`, `ch04`, …) snapshots the editor at that
point in the tutorial. `main` equals `ch17` (the last teaching chapter).

```
git tag -l 'ch*'
git checkout ch05   # rewind to the "read a hardcoded file" chapter
```
