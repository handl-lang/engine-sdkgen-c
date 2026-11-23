# The `c` backend — C headers via libclang

`handl-plugin-codegen-backend-c` parses a C header through libclang and
produces the generic `SdkgenModule` IR; the `hnl-file` emitter
(`engine-codegen-file`) renders it. Framework context:
[`host/crates/plugin-codegen`](https://github.com/handl-lang/host/tree/main/crates/plugin-codegen).

```bash
hnl sdkgen c /usr/include/time.h --output handl-packages/generated/libc \
    [--target x86_64-unknown-linux-gnu] [--include 'clock_*'] \
    [--exclude '…'] [--config libc.sdkgen.yaml]
```

`--target` is a libclang triple (default: the host triple);
`--include`/`--exclude` are comma-separated globs (`source_filters:
true` on the target record).

## Pipeline

`Source<SdkgenModule>::parse` on `CBackend` (`src/backend/mod.rs`):

1. **TU setup** — `clang_parseTranslationUnit` with `-x c
   -fparse-all-comments -iquote <header dir> -I/usr/include
   [-resource-dir <clang's>] -target <triple> [-D<define>…]` and
   `CXTranslationUnit_DetailedPreprocessingRecord` (without it macro
   cursors never appear). `-iquote`, not `-I`, for the header dir so a
   spec dir can't shadow system headers by basename.
   Error/fatal libclang diagnostics fail the parse; warnings are
   tolerated.
2. **Collect** (`collect.rs`) — top-level cursors bucketed into
   functions/records/typedefs/enums/macros:
   - functions: main-file only, external linkage, `include`/`exclude`
     gate.
   - records/typedefs: main file plus system headers (they supply the
     ABI for selected functions); `exclude` vetoes everywhere; only
     struct *definitions* (forward decls have no layout).
   - enums: exempt from `include` at decl level (anonymous enums carry
     public constants); constants filtered per-name in `collect_enum`.
   - object-like `#define`s: `_`-prefixed names and builtin origins are
     never API; `constants:` globs win outright, else include/exclude,
     else the header's whole include closure.
3. **Parse** (`parse/`, `map/`) — each cursor lowers to `Record`,
   `Typedef`, or `Function` IR; per-symbol config overrides apply; a
   failed parse is a `module.diagnostics` entry unless a full
   `symbols.<name>.args` override *rescues* the signature.
4. **Macro probe** (`macro_probe.rs`) — libclang can't evaluate
   `MacroDefinition` cursors, so a synthesized TU (`#include` + `enum {
   __hnl_sdkgen_<NAME> = (<NAME>) };` per macro, as an unsaved buffer)
   folds them. Non-integer macros (header guards, strings) silently
   drop. Docs come from the main TU's comment/source text.
5. **Prune** (`prune.rs`) — records/typedefs not referenced by an
   emitted function are dropped (no-op on types-only runs); `rec:`
   fields pointing at pruned/skipped records degrade their port to
   `Handle` (byte size keeps the ABI right).
6. **Imports** (`imports.rs`) — `types/c/<header>` aliases referenced
   by surviving port types are declared on `module.imports` as data.

## C → IR mapping

Scalar args (`map/scalar.rs`): sub-32-bit ints widen to `i32` (full
arg slot on the supported ABIs); `long` widths come from
`clang_Type_getSizeOf` (LLP64/ILP32 vs LP64, no LP64 assumption);
`double` → `Float`/`f64` (`float` is rejected — an `f64` descriptor
would misplace the bits); enums → `Int`/`i32`. Record fields marshal at
real width (`i8`…`u64`, `f64`). Returns: `void` → `()`/`void`,
`void *` → `Handle`/`handle`, `char *` → `String`/`cstr`.

Known C typedef spellings get `types/c` port aliases while the
descriptor keeps the real width — `size_t` → `Size` (`types/c/stddef`),
`clockid_t` → `ClockId` (`types/c/time`), `socklen_t` → `Socklen`
(`types/c/socket`), etc. (the `CTYPES` table in `map/scalar.rs`).

Pointer args (`map/arg.rs`):

| C shape | Descriptor | Port |
| --- | --- | --- |
| `const void *` + len | `in_buf` | `Bytes` (one arg for the pair) |
| `const char *` + len | `cbytes` | `Bytes` |
| `const char *` | `cstr` | `String` |
| `const struct T *` + len | `in_buf` | `Bytes` |
| `const struct T *` | `in_struct:{…}` | the record |
| `char *` | `cstr` | `String` |
| `char *` + len | — | **rejected** (a `cstr` would drop written data) |
| `void *` (mut) | `ptr` | `Handle` |
| `struct T *` | `out_struct:{…}` | no arg; `{Port}Result.buf` carries it |
| `struct T *` opaque/unbuildable | `ptr` | `Handle` fallback |

The `(ptr, len)` collapse: a trailing arg counts as the length when it
is `size_t` — or, for non-variadic functions only, len-like by
type (`ssize_t`, `socklen_t`, `off_t`, `long`) or name
(`len`/`*len`/`*size`/`*count`/`n`/`nbytes`/`capacity`/`num`). The
variadic carve-out keeps `open(const char*, int, …)`'s `flags` from
being read as a length.

Struct descriptors carry real layout: `in_struct:{name:kind@off[:size]}`
with offsets/widths from `clang_Cursor_getOffsetOfField`/
`Type_getSizeOf`; nested `rec:` fields expand inline, unknown nested
records degrade to `arr:<size>:u8`.

Record fields: `T[N]` → `arr:N:elem`, pointer → `ptr`, inline struct →
`rec:<name>`. Bitfields and anonymous structs are rejected (the
descriptor vocabulary can't express their layout).

A `Function` fills `arg_descriptors` one-per-C-arg, `port_args` only
for descriptors that consume a Handl arg, `out_struct` with the record
name when auto-mapped (→ return type `{Port}Result`), `errno_guard:
true` by default, `c_decl` the C signature spelling.

## `.sdkgen.yaml` — `SdkgenConfig`

```yaml
library: libc.so.6            # shared object adapters call
defines: [_GNU_SOURCE]        # extra -D flags for libclang
constants: ["EPOLL*"]         # glob gate on enum constants + object-like macros
headers:                      # per-header narrowing, keyed by basename
  unistd.h: { include: […], exclude: […] }
symbols:
  open: { args: [cstr, i32], ret: Int, skip: false, guard: true, names: [path, flags] }
docs: { clock_gettime: "…" }  # hand-written docs — beat comments and man pages
external_docs: false          # disable the man-page layer
guard: false                  # default errno_guard for every function
```

| Key | Effect |
| --- | --- |
| `library` | `module.library` — the `.so` adapters pass to `ffi.call` |
| `defines` | `-D` args (feature-test gates like `_GNU_SOURCE`) |
| `constants` | Globs for enum constants and macros; empty = everything in the include closure |
| `headers.<basename>.{include,exclude}` | Globs **unioned** with `--include`/`--exclude` when the input basename matches — makes regen recipes reproducible from config |
| `symbols.<name>.args` | Wholesale descriptor override; clears `out_struct`; `port_args` are rebuilt from consuming descriptors (names recovered by C-arg index, `__arg_<i>` fallback) |
| `symbols.<name>.names` | Rename the consuming port args, in order |
| `symbols.<name>.ret` / `.descriptor` | Override return port / return descriptor (`ret` implies a mapped descriptor; `descriptor` wins) |
| `symbols.<name>.skip` | Drop the symbol |
| `symbols.<name>.guard` | Per-function `errno_guard` (beats `guard:`) |
| `docs.<key>` | Doc overrides keyed by symbol / `record.field` / constant |
| `external_docs` | `false` disables man-page lookup |
| `guard` | Default `errno_guard`; set `false` for out-of-band error channels (`UErrorCode*`, Win32) so valid negative returns don't sprout a spurious `ret < 0` check |

## Doc layering

The target's `docs` pass is `CBackend::apply_docs_with` —
`pick_doc` per slot: `docs:` override → header comment (`-fparse-all-comments`
captures plain `/* */`, not just doxygen) → `ManPageDocSource` (local
`/usr/share/man` page files incl. `.so` alias hops → `man` viewer →
man7.org over HTTP; sections 2 and 3, NAME-section description).
Field docs come from overrides or field comments only — `record.field`
is not a man-page key.

## What it leaves to the emitter

`client_package = "ffi"`, `backend_id = "c"`, `module` from the header
stem, `library` only from `config.library` (empty → emitter defaults
`<lib>.so.6`). `per_record_files` stays off — C records reference each
other by name and must share one module scope.
