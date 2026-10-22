# sdkgen-c

`handl-plugin-sdkgen-backend-c` — C / libclang backend for the Handl SDK generator (ADR-0107).

Sibling-repo deps are git dependencies (`branch = "main"`); `[patch]` tables
resolve them from sibling checkouts.

## Docs

- [docs/index.md](docs/index.md) — the `c` backend: libclang pipeline,
  C → IR mapping rules, `.sdkgen.yaml` (`SdkgenConfig`) reference, and
  the descriptor vocabulary it emits.
- Framework: [`host/docs/sdkgen/`](https://github.com/handl-lang/host/blob/main/docs/sdkgen/index.md).
