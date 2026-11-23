//! C / libclang backend for the Handl codegen pipeline (ADR-0107, PL-004).
//!
//! Pipeline, in module order:
//! - [`backend`] — `CBackend`: libclang translation-unit setup and the
//!   `Source::parse` driver (`clang_args` → TU →
//!   `diagnostics` → collect → parse → `macro_probe` → `prune` →
//!   `imports`), plus `apply_docs` doc layering (`docs`, `man`).
//! - [`cx`] — thin safe wrappers over libclang (strings, locations,
//!   visitor helpers).
//! - [`collect`] — top-level declaration collection honoring `include`/`exclude`.
//! - [`parse`] — lowers cursors into `SdkgenModule` IR (`Record`,
//!   `Typedef`, `Function`), including `(ptr, len)` pair collapse and
//!   `out_struct` resolution.
//! - [`map`] — C type → handl port + FFI descriptor assignment.
//! - [`config`] — `.sdkgen.yaml` overrides (`SdkgenConfig`).
//!
//! `testutil` (test-only) parses C snippets into live TUs so `map`/`parse`
//! internals are unit-testable; `tests/` holds end-to-end fixtures.

#![allow(non_upper_case_globals)]

mod backend;
mod collect;
mod config;
mod cx;
mod map;
mod parse;

pub use backend::{CBackend, ManPageDocSource};
pub use config::{SdkgenConfig, SymbolOverride};

use handl_plugin_codegen::{
    AnySource, CodegenRegistry, CodegenRequest, CodegenSourceDef, CodegenTarget, SdkgenModule,
    erase_source,
};

/// Register the `c` codegen target on `reg` (ADR-0128 §0.1, PL-004):
/// source `c` plus the target record `(source: "c", emitter: "hnl-file")`
/// — the emitter resolves by name through the registry, so this module
/// never crate-depends on the emitter's repo.
pub fn register(reg: &mut CodegenRegistry) {
    reg.register_source(CodegenSourceDef {
        name: "c",
        make: make_source,
    });
    reg.register_target(CodegenTarget {
        name: "c",
        source: "c",
        emitter: "hnl-file",
        source_filters: true,
        docs: Some(apply_docs),
        spec_input: None,
    });
}

fn make_source(req: &CodegenRequest<'_>) -> Result<Box<dyn AnySource>, String> {
    let config = match req.config {
        Some(path) => SdkgenConfig::load(path)?,
        None => SdkgenConfig::default(),
    };
    let backend = CBackend::new(
        req.target.map(str::to_string),
        config,
        req.include.map(str::to_string),
        req.exclude.map(str::to_string),
    )
    .map_err(|e| e.to_string())?;
    Ok(erase_source(backend))
}

fn apply_docs(req: &CodegenRequest<'_>, module: &mut SdkgenModule) -> Result<(), String> {
    let config = match req.config {
        Some(path) => SdkgenConfig::load(path)?,
        None => SdkgenConfig::default(),
    };
    CBackend::apply_docs_with(&config, module, None);
    Ok(())
}

#[cfg(test)]
mod collect_test;
#[cfg(test)]
mod config_test;
#[cfg(test)]
mod testutil;
