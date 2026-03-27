# Contributing

## Local setup

1. Install Rust from https://rustup.rs/
2. Install `fzf`
3. Install `codex` and/or `claude`
4. Run `cargo test`

## Local install

```sh
./install.sh
```

Or on Windows:

```powershell
.\install.ps1
```

## Notes

- The public commands are `csx` and `clx`
- The legacy alias `cxs` still maps to `csx`
- The Rust runtime lives under `src/*.rs`
- `cargo test` is the test entrypoint
