# Agent guide — tool-codegen-c

`handl-plugin-codegen-backend-c` — the C / libclang backend for the
Handl codegen pipeline (`tool-codegen-*` family, PL-004 T-075).
Consumes specs (`*.sdkgen.yaml`, `SdkgenConfig`) and produces
`SdkgenModule` IR for `Source<SdkgenModule>` / `Emitter<SdkgenModule>`
targets. Rust 2024.

## Layout

| path | role |
| --- | --- |
| `src/lib.rs` | crate docs + public API |
| `src/backend/` | libclang pipeline — `clang_args`, `diagnostics`, `docs`, `imports`, `macro_probe`, `man`, `prune` (with `*_test.rs` siblings) |
| `src/cx/` | clang-cursor helpers — `visit`, `location`, `strings` |
| `src/collect.rs`, `src/config.rs` | spec collection + `SdkgenConfig` |
| `src/parse/`, `src/map/` | C parsing and C → IR mapping rules |
| `tests/c_backend.rs` + `tests/fixtures/` | backend contract tests |
| `docs/` | backend doc — libclang pipeline, mapping rules, `.sdkgen.yaml` reference |
| `xtask/`, `workflows/gate.yaml`, `bupha.toml`, `.cargo/` | vendored repo gate + CI wiring (`cargo xtask` alias) |

## Contract

- Deps resolve from git (`handl-plugin-codegen` → `handl-lang/host`
  `branch = "main"`); committed `[patch]` tables map engine/kernel/host
  git deps to sibling checkouts locally.
- Refusals/diagnostics are named (`src/backend/diagnostics.rs`) — the
  backend reports, never silently drops, unmappable C constructs.

## Verification

```bash
cargo xtask gate   # fmt --check → clippy -D warnings → test → doc
```


<!-- myco:agent-awareness -->

## Agent awareness

- Session start / any work discussion: run `myco status --prompt`
  before proposing next steps — know the board first.
- User describes new work → offer to capture it (`myco todo add`,
  `issue new`, `proposal new`). Don't wait to be asked.
- Picking up a task: `myco resume <ID>` is the briefing.
- Wrapping up: `myco handoff <ID>` anything resumable; keep statuses
  honest (claimed → in-progress → blocked/done).

<!-- /myco:agent-awareness -->
