# engine-codegen-c

`handl-plugin-codegen-backend-c` — C / libclang backend for the Handl
codegen pipeline. Part of the `engine-codegen-*` family (PL-004 T-075):
consumes specs (`*.sdkgen.yaml`, `SdkgenConfig`) and produces the
`SdkgenModule` IR for `Source<SdkgenModule>` / `Emitter<SdkgenModule>`
targets.

## Development

```bash
cargo xtask gate        # fmt --check → clippy -D warnings → test → doc
```

`cargo xtask` wraps the repo's `xtask/` crate; `ci.sh` is the gate entry.
Deps resolve from git, so no sibling checkouts are needed; a plain clone
test-runs as-is.

## Docs

- [docs/index.md](docs/index.md) — the `c` backend: libclang pipeline,
  C → IR mapping rules, `.sdkgen.yaml` (`SdkgenConfig`) reference, and
  the descriptor vocabulary it emits.
- Framework: [`host/crates/plugin-codegen`](https://github.com/handl-lang/host/tree/main/crates/plugin-codegen).

## License

MIT.
