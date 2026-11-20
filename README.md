# tool-sdkgen-c

`handl-plugin-sdkgen-backend-c` — C / libclang backend for the Handl SDK
generator. Part of the `tool-sdkgen-*` family: consumes Handl SDK specs
(`*.sdkgen.yaml`, `SdkgenConfig`) and emits C bindings/SDK sources.

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
- Framework: [`host/docs/sdkgen/`](https://github.com/handl-lang/host/blob/main/docs/sdkgen/index.md).

## License

MIT.
