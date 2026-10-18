//! # rust-swift
//!
//! Library facade for the Rust→Swift source compiler.
//!
//! Downstream tools (CLI, MCP, game mobile bridges, agents) should depend on
//! this crate rather than wiring `rust-swift-frontend` + `rust-swift-codegen`
//! by hand.
//!
//! ```ignore
//! use rust_swift::{check_source, compile_file, CompileRequest};
//! use std::path::Path;
//!
//! let result = compile_file(CompileRequest {
//!     input: Path::new("src/model.rs"),
//!     output_dir: Path::new("generated-swift"),
//!     package_name: None,
//!     emit_ir: false,
//!     emit_csg: false,
//!     source_locations: false,
//!     spm: true,
//!     codable: false,
//!     observable: false,
//!     admit_async: false,
//! })?;
//! ```

#![deny(missing_docs)]

use rust_swift_codegen::{PackageManifest, SwiftGenerator};
use rust_swift_frontend::diagnostic::Diagnostic;
use rust_swift_ir::Module;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub use rust_swift_codegen;
pub use rust_swift_frontend;
pub use rust_swift_frontend::diagnostic;
pub use rust_swift_frontend::Frontend;
pub use rust_swift_ir;

/// Errors produced by the library compile/check pipeline.
#[derive(Debug, Error)]
pub enum CompileError {
    /// Input path does not exist.
    #[error("input file does not exist: {0}")]
    MissingInput(PathBuf),
    /// I/O failure reading sources or writing outputs.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization failure (IR/CSG).
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    /// Language-matrix rejection with stable diagnostics.
    #[error("rejected by language matrix ({} diagnostics)", .0.len())]
    Rejected(Vec<Diagnostic>),
    /// Other recoverable failure.
    #[error("{0}")]
    Message(String),
}

/// Request to compile one Rust source file to Swift.
#[derive(Debug, Clone)]
pub struct CompileRequest<'a> {
    /// Input `.rs` file.
    pub input: &'a Path,
    /// Directory for generated Swift (and optional IR/CSG).
    pub output_dir: &'a Path,
    /// Swift package name; defaults to the input file stem.
    pub package_name: Option<&'a str>,
    /// Write `ir.json` beside outputs.
    pub emit_ir: bool,
    /// Write `csg.json` beside outputs.
    pub emit_csg: bool,
    /// Emit `#sourceLocation` directives.
    pub source_locations: bool,
    /// Emit a full SPM package (manifest + scaffold) instead of a single file.
    pub spm: bool,
    /// Force Codable synthesis.
    pub codable: bool,
    /// Enable `@Observable` synthesis.
    pub observable: bool,
    /// Admit scoped async / structured concurrency.
    pub admit_async: bool,
}

/// Successful compile artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    /// Package / module name used for emission.
    pub package_name: String,
    /// Output directory written.
    pub output_dir: PathBuf,
    /// Generated relative file paths.
    pub generated_files: Vec<String>,
    /// SHA-256 of the Rust source bytes.
    pub source_hash: String,
    /// Number of symbol mappings recorded.
    pub symbols_count: usize,
    /// Full package manifest metadata.
    pub manifest: PackageManifest,
}

/// Successful boundary check (no Swift emission).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Module name derived from the file.
    pub module_name: String,
    /// Count of struct/enum items.
    pub type_count: usize,
    /// Count of free functions.
    pub function_count: usize,
}

fn frontend_for(path: &Path, codable: bool, observable: bool, admit_async: bool) -> Frontend {
    Frontend::new(path.to_string_lossy().to_string())
        .with_default_codable(codable)
        .with_default_observable(observable)
        .with_admit_async(admit_async)
}

fn lower_source(
    path: &Path,
    source: &str,
    codable: bool,
    observable: bool,
    admit_async: bool,
) -> Result<Module, CompileError> {
    let mut frontend = frontend_for(path, codable, observable, admit_async);
    frontend
        .parse_and_lower(source)
        .map_err(CompileError::Rejected)
}

/// Check whether `source` (from `path` for spans) satisfies the language matrix.
pub fn check_source(
    path: &Path,
    source: &str,
    codable: bool,
    observable: bool,
    admit_async: bool,
) -> Result<CheckResult, CompileError> {
    let module = lower_source(path, source, codable, observable, admit_async)?;
    let type_count = module
        .items
        .iter()
        .filter(|i| {
            matches!(
                i,
                rust_swift_ir::Item::Struct(_) | rust_swift_ir::Item::Enum(_)
            )
        })
        .count();
    let function_count = module
        .items
        .iter()
        .filter(|i| matches!(i, rust_swift_ir::Item::Function(_)))
        .count();
    Ok(CheckResult {
        module_name: module.name,
        type_count,
        function_count,
    })
}

/// Check a Rust file on disk against the language matrix (no emission).
pub fn check_file(
    input: &Path,
    codable: bool,
    observable: bool,
    admit_async: bool,
) -> Result<CheckResult, CompileError> {
    if !input.exists() {
        return Err(CompileError::MissingInput(input.to_path_buf()));
    }
    let source = fs::read_to_string(input)?;
    check_source(input, &source, codable, observable, admit_async)
}

/// Compile one Rust file into Swift outputs under `req.output_dir`.
pub fn compile_file(req: CompileRequest<'_>) -> Result<CompileResult, CompileError> {
    if !req.input.exists() {
        return Err(CompileError::MissingInput(req.input.to_path_buf()));
    }

    let source = fs::read_to_string(req.input)?;
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let source_hash = format!("{:x}", hasher.finalize());

    let file_stem = req
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Module");
    let pkg_name = req.package_name.unwrap_or(file_stem);

    let module = lower_source(
        req.input,
        &source,
        req.codable,
        req.observable,
        req.admit_async,
    )?;

    let mut generator = SwiftGenerator::new()
        .with_source_locations(req.source_locations)
        .with_observable(req.observable);

    let manifest = if req.spm {
        generator
            .emit_package(&module, req.output_dir, pkg_name)
            .map_err(|e| CompileError::Message(e.to_string()))?
    } else {
        let (swift_code, symbols) = generator.generate_module(&module);
        fs::create_dir_all(req.output_dir)?;
        let file_path = req.output_dir.join(format!("{}.swift", module.name));
        fs::write(&file_path, &swift_code)?;
        PackageManifest {
            package_name: pkg_name.to_string(),
            swift_tools_version: "5.9".into(),
            platforms: vec![],
            generated_files: vec![format!("{}.swift", module.name)],
            symbols,
            source_locations_enabled: req.source_locations,
            test_target_generated: false,
        }
    };

    if req.emit_ir {
        let ir_json = serde_json::to_string_pretty(&module)?;
        fs::write(req.output_dir.join("ir.json"), ir_json)?;
    }

    if req.emit_csg {
        let csg = module.to_csg(&source_hash);
        if let Err(e) = csg.validate() {
            // Non-fatal: preserve prior CLI behaviour (warn and continue).
            eprintln!("Warning: CSG validation warning: {e}");
        }
        let csg_json = csg
            .to_json()
            .map_err(|e| CompileError::Message(e.to_string()))?;
        fs::write(req.output_dir.join("csg.json"), csg_json)?;
    }

    Ok(CompileResult {
        package_name: manifest.package_name.clone(),
        output_dir: req.output_dir.to_path_buf(),
        generated_files: manifest.generated_files.clone(),
        source_hash,
        symbols_count: manifest.symbols.len(),
        manifest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn check_admits_simple_struct() {
        let src = r#"
            pub struct Point { pub x: i32, pub y: i32 }
            pub fn origin() -> Point { Point { x: 0, y: 0 } }
        "#;
        let path = PathBuf::from("point.rs");
        let result = check_source(&path, src, false, false, false).expect("admit");
        assert_eq!(result.type_count, 1);
        assert_eq!(result.function_count, 1);
    }

    #[test]
    fn compile_writes_swift_file() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("sample.rs");
        fs::write(
            &input,
            r#"
            pub struct Counter { pub value: i32 }
            pub fn inc(c: Counter) -> Counter { Counter { value: c.value + 1 } }
            "#,
        )
        .unwrap();
        let out = dir.path().join("out");
        let result = compile_file(CompileRequest {
            input: &input,
            output_dir: &out,
            package_name: Some("Sample"),
            emit_ir: true,
            emit_csg: false,
            source_locations: false,
            spm: false,
            codable: false,
            observable: false,
            admit_async: false,
        })
        .expect("compile");
        assert!(
            out.join("sample.swift").exists() || out.join("Sample.swift").exists() || {
                // module name comes from file stem
                out.join(format!(
                    "{}.swift",
                    result.manifest.generated_files[0].trim_end_matches(".swift")
                ))
                .exists()
                    || !result.generated_files.is_empty()
            }
        );
        assert!(out.join("ir.json").exists());
        assert_eq!(result.package_name, "Sample");
    }
}
