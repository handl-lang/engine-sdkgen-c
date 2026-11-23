//! The `CBackend` entry point: libclang translation-unit setup and the
//! `Source::parse_header` driver.
//!
//! Pipeline: [`clang_args`] → parse TU → [`diagnostics`] gate on
//! error/fatal → [`crate::collect`] declaration cursors →
//! [`crate::parse`] each into IR with config overrides →
//! [`macro_probe`] evaluates `#define` constants → [`prune`] drops
//! unreferenced types and repairs dangling `rec:` fields →
//! [`imports`] declares `types/c` imports → `SdkgenModule`.
//! [`docs`] holds `CBackend::apply_docs`, the doc-layering pass the
//! caller runs separately on the finished module.

mod clang_args;
mod diagnostics;
mod docs;
mod imports;
mod macro_probe;
mod man;
mod prune;

#[cfg(test)]
mod clang_args_test;
#[cfg(test)]
mod diagnostics_test;
#[cfg(test)]
mod macro_probe_test;
#[cfg(test)]
mod man_test;
#[cfg(test)]
mod prune_test;

use clang_sys::*;
use glob::Pattern;
use handl_plugin_codegen::{CodegenError, EmitContext, SdkgenModule, Source};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CString, c_int, c_void};
use std::path::Path;

use crate::collect::{CursorData, collect_cursors, collect_enum};
use crate::config::SdkgenConfig;
use crate::cx::cursor_name;
use crate::parse::{ParseCtx, parse_function, parse_record, parse_typedef};

pub use man::ManPageDocSource;

/// C header parser using libclang.
pub struct CBackend {
    target: Option<String>,
    config: SdkgenConfig,
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
    consts: Vec<Pattern>,
}

/// Compile a comma-separated glob list (`"a*,b?,c"`) into patterns.
fn globs(list: &str) -> Result<Vec<Pattern>, CodegenError> {
    list.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            Pattern::new(s).map_err(|e| CodegenError::Parse(format!("invalid glob `{s}`: {e}")))
        })
        .collect()
}

impl CBackend {
    pub fn new(
        target: Option<String>,
        config: SdkgenConfig,
        include: Option<String>,
        exclude: Option<String>,
    ) -> Result<Self, CodegenError> {
        let include = include.map(|p| globs(&p)).transpose()?.unwrap_or_default();
        let exclude = exclude.map(|p| globs(&p)).transpose()?.unwrap_or_default();
        let consts = config
            .constants
            .iter()
            .map(|s| globs(s))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(Self {
            target,
            config,
            include,
            exclude,
            consts,
        })
    }
}

impl Source<SdkgenModule> for CBackend {
    /// Lift a C header into [`SdkgenModule`] via libclang. Pure: parses
    /// the TU, writes nothing. The header path comes from
    /// `EmitContext::spec_path`; `source_url` is stamped as provenance.
    fn parse(&self, req: &EmitContext) -> Result<SdkgenModule, CodegenError> {
        let path = req.spec_path.as_deref().ok_or_else(|| {
            CodegenError::Parse("c source requires EmitContext::spec_path".into())
        })?;
        self.parse_header_inner(path, req.source_url.as_deref())
    }
}

impl CBackend {
    /// Convenience wrapper for direct/test use — the pipeline entry
    /// point is [`Source::parse`] over an [`EmitContext`].
    pub fn parse_header(&self, path: &Path) -> Result<SdkgenModule, CodegenError> {
        self.parse_header_inner(path, None)
    }

    fn parse_header_inner(
        &self,
        path: &Path,
        source_url: Option<&str>,
    ) -> Result<SdkgenModule, CodegenError> {
        if !path.is_file() {
            return Err(CodegenError::Parse(format!(
                "header not found: {}",
                path.display()
            )));
        }
        clang_sys::load().map_err(|e| CodegenError::Parse(format!("libclang load failed: {e}")))?;

        let target = self
            .target
            .clone()
            .unwrap_or_else(|| target_lexicon::Triple::host().to_string());

        let index = unsafe { clang_createIndex(0, 0) };
        if index.is_null() {
            return Err(CodegenError::Parse("libclang createIndex failed".into()));
        }

        // `_raw_args` owns the CStrings `args` points into — must outlive
        // every libclang call that takes `args` (incl. the macro probe).
        let (_raw_args, args) = clang_args::clang_args(path, &target, &self.config.defines);
        let c_path = CString::new(path.to_string_lossy().as_bytes()).unwrap();
        let tu = unsafe {
            clang_parseTranslationUnit(
                index,
                c_path.as_ptr(),
                args.as_ptr(),
                args.len() as c_int,
                std::ptr::null_mut(),
                0,
                // DetailedPreprocessingRecord: without it macro cursors
                // never appear in the translation unit's child list.
                CXTranslationUnit_DetailedPreprocessingRecord,
            )
        };
        if tu.is_null() {
            unsafe { clang_disposeIndex(index) };
            return Err(CodegenError::Parse(
                "libclang failed to parse header".into(),
            ));
        }
        if let Err(e) = diagnostics::check_tu(tu) {
            unsafe { clang_disposeTranslationUnit(tu) };
            unsafe { clang_disposeIndex(index) };
            return Err(e);
        }

        let pointer_width = unsafe {
            let ti = clang_getTranslationUnitTargetInfo(tu);
            let width = clang_TargetInfo_getPointerWidth(ti);
            clang_TargetInfo_dispose(ti);
            width as usize
        };
        let ctx = ParseCtx {
            pointer_width,
            records: RefCell::new(HashMap::new()),
        };

        // Config `headers:` per-file include/exclude patterns union with
        // `--include`/`--exclude`, so a narrowed header's recipe is
        // reproducible from the yaml.
        let mut include = self.include.clone();
        let mut exclude = self.exclude.clone();
        let basename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if let Some(hc) = self.config.headers.get(basename) {
            for pat in &hc.include {
                include.extend(globs(pat)?);
            }
            for pat in &hc.exclude {
                exclude.extend(globs(pat)?);
            }
        }

        let mut cursor_data = CursorData {
            include: include.clone(),
            exclude: exclude.clone(),
            consts: self.consts.clone(),
            ..Default::default()
        };
        let root = unsafe { clang_getTranslationUnitCursor(tu) };
        unsafe {
            clang_visitChildren(
                root,
                collect_cursors,
                &mut cursor_data as *mut _ as *mut c_void,
            )
        };

        let mut module = SdkgenModule {
            client_package: "ffi".into(),
            backend_id: "c".into(),
            target,
            ..Default::default()
        };
        if let Some(lib) = &self.config.library {
            module.library = lib.clone();
        }
        module.module = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module")
            .to_string();

        for &cursor in &cursor_data.records {
            match parse_record(cursor, &ctx) {
                Ok(rec) => {
                    if !self.config.should_skip(&rec.name) {
                        ctx.records
                            .borrow_mut()
                            .insert(rec.name.clone(), rec.clone());
                        module.records.push(rec);
                    }
                }
                Err(e) => module.diagnostics.push(e),
            }
        }

        for &cursor in &cursor_data.typedefs {
            match parse_typedef(cursor, &ctx) {
                Ok(td) => {
                    if !self.config.should_skip(&td.name) {
                        module.typedefs.push(td);
                    }
                }
                Err(e) => module.diagnostics.push(e),
            }
        }

        for &cursor in &cursor_data.enums {
            collect_enum(cursor, &mut module, &include, &exclude, &self.consts);
        }

        let (macro_names, macro_docs) = macro_probe::macro_docs(&cursor_data.macros);
        for mut c in macro_probe::eval_macros(index, &args, &macro_names, path) {
            if !module.enum_constants.iter().any(|e| e.name == c.name) {
                c.doc = macro_docs.get(&c.name).cloned().flatten();
                module.enum_constants.push(c);
            }
        }

        for &cursor in &cursor_data.functions {
            match parse_function(cursor, &ctx) {
                Ok(mut f) => {
                    if !self.config.should_skip(&f.name) {
                        let name = f.name.clone();
                        self.config.apply_to_function(&name, &mut f);
                        module.functions.push(f);
                    }
                }
                Err(e) => {
                    // A full `args` override can rescue a symbol whose C
                    // signature we cannot parse (e.g. `char**` params).
                    let fname = cursor_name(cursor);
                    match self.config.rescue_function(&fname) {
                        Some(mut f) if !self.config.should_skip(&f.name) => {
                            // The signature came from config, but the
                            // declaration cursor still carries its own doc
                            // comment.
                            f.doc = crate::cx::doc_comment(cursor);
                            module.functions.push(f);
                        }
                        _ => module.diagnostics.push(e),
                    }
                }
            }
        }

        prune::prune_unreferenced(&mut module);
        prune::fix_dangling_rec_fields(&mut module);
        imports::collect_type_imports(&mut module);
        module.source = source_url.map(str::to_string);

        unsafe {
            clang_disposeTranslationUnit(tu);
            clang_disposeIndex(index);
        }

        Ok(module)
    }
}
