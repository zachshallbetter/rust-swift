//! Incremental Compiler Semantic Graph (CSG) Synchronizer & File Watcher Daemon.
//!
//! Provides delta synchronization between Rust source files and emitted Swift packages:
//! - Tracks SHA-256 and Compiler Semantic Graph (CSG) digests per compilation unit
//! - Avoids redundant Swift code generation when source changes preserve semantic AST invariant
//! - Supports single-pass (`--once`) and continuous daemon monitoring (`--watch`)
//! - Appends telemetry receipts to `records/sync.jsonl`

use rust_swift_codegen::SwiftGenerator;
use rust_swift_frontend::Frontend;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    SyncedUpdated,
    SkippedInvariant,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRecord {
    pub timestamp: String,
    pub source_file: String,
    pub previous_csg_hash: Option<String>,
    pub current_csg_hash: String,
    pub action: SyncAction,
    pub duration_ms: u64,
    pub details: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub file_csg_hashes: HashMap<String, String>,
}

pub struct SyncEngine {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub state: SyncState,
}

impl SyncEngine {
    pub fn new(input_dir: impl Into<PathBuf>, output_dir: impl Into<PathBuf>) -> Self {
        let input_dir = input_dir.into();
        let output_dir = output_dir.into();
        let mut state = SyncState::default();

        let cache_path = output_dir.join(".rust-swift-sync-cache.json");
        if cache_path.exists() {
            if let Ok(data) = fs::read_to_string(&cache_path) {
                if let Ok(loaded) = serde_json::from_str::<SyncState>(&data) {
                    state = loaded;
                }
            }
        }

        Self {
            input_dir,
            output_dir,
            state,
        }
    }

    /// Performs one synchronization pass across all .rs files in input_dir.
    pub fn sync_once(&mut self) -> Result<Vec<SyncRecord>, Box<dyn std::error::Error>> {
        let mut records = Vec::new();
        let rs_files = self.discover_rust_files(&self.input_dir.clone())?;

        for rs_file in rs_files {
            let record = self.sync_file(&rs_file)?;
            records.push(record);
        }

        self.persist_records(&records)?;
        Ok(records)
    }

    /// Synchronizes an individual .rs file.
    pub fn sync_file(
        &mut self,
        file_path: &Path,
    ) -> Result<SyncRecord, Box<dyn std::error::Error>> {
        let start = Instant::now();
        let source = fs::read_to_string(file_path)?;
        let file_key = file_path.to_string_lossy().to_string();

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("module.rs");
        let pkg_name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Package");

        let mut frontend = Frontend::new(file_name);
        let module = match frontend.parse_and_lower(&source) {
            Ok(m) => m,
            Err(diags) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return Ok(SyncRecord {
                    timestamp: "2026-10-14T15:45:00Z".into(),
                    source_file: file_key.clone(),
                    previous_csg_hash: self.state.file_csg_hashes.get(&file_key).cloned(),
                    current_csg_hash: String::new(),
                    action: SyncAction::Error,
                    duration_ms,
                    details: format!("Frontend parse error: {} diagnostics", diags.len()),
                });
            }
        };

        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        let source_sha = format!("{:x}", hasher.finalize());

        let csg = module.to_csg(&source_sha);
        let csg_json = serde_json::to_string(&csg).unwrap_or_default();
        let mut csg_hasher = Sha256::new();
        csg_hasher.update(csg_json.as_bytes());
        let current_csg_hash = format!("{:x}", csg_hasher.finalize());

        let prev_hash = self.state.file_csg_hashes.get(&file_key).cloned();

        let (action, details) = if let Some(ref prev) = prev_hash {
            if prev == &current_csg_hash {
                (
                    SyncAction::SkippedInvariant,
                    "CSG digest is unchanged. Rebuild skipped.".into(),
                )
            } else {
                self.emit_package(&module, pkg_name)?;
                self.state
                    .file_csg_hashes
                    .insert(file_key.clone(), current_csg_hash.clone());
                (
                    SyncAction::SyncedUpdated,
                    "CSG delta detected. Generated Swift package updated.".into(),
                )
            }
        } else {
            self.emit_package(&module, pkg_name)?;
            self.state
                .file_csg_hashes
                .insert(file_key.clone(), current_csg_hash.clone());
            (
                SyncAction::SyncedUpdated,
                "Initial synchronization. Generated Swift package emitted.".into(),
            )
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(SyncRecord {
            timestamp: "2026-10-14T15:45:00Z".into(),
            source_file: file_key,
            previous_csg_hash: prev_hash,
            current_csg_hash,
            action,
            duration_ms,
            details,
        })
    }

    /// Continuous file watcher loop with configurable polling interval.
    pub fn watch_loop(
        &mut self,
        interval_ms: u64,
        max_cycles: Option<usize>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cycle = 0;
        println!(
            "rust-swift-sync daemon active. Watching {} -> {}",
            self.input_dir.display(),
            self.output_dir.display()
        );
        loop {
            let records = self.sync_once()?;
            for r in &records {
                if r.action == SyncAction::SyncedUpdated {
                    let short_hash = if r.current_csg_hash.len() >= 8 {
                        &r.current_csg_hash[..8]
                    } else {
                        &r.current_csg_hash
                    };
                    println!(
                        "[SYNC] Updated: {} (CSG: {}) in {}ms",
                        r.source_file, short_hash, r.duration_ms
                    );
                }
            }
            cycle += 1;
            if let Some(max) = max_cycles {
                if cycle >= max {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(interval_ms));
        }
        Ok(())
    }

    fn emit_package(
        &self,
        module: &rust_swift_ir::Module,
        package_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut gen = SwiftGenerator::new();
        gen.emit_package(module, &self.output_dir, package_name)?;
        Ok(())
    }

    fn discover_rust_files(&self, dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut files = Vec::new();
        if dir.is_file() {
            if dir.extension().and_then(|s| s.to_str()) == Some("rs") {
                files.push(dir.to_path_buf());
            }
            return Ok(files);
        }

        if !dir.exists() {
            return Ok(files);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                if !name.starts_with('.') && name != "target" {
                    files.extend(self.discover_rust_files(&path)?);
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                files.push(path);
            }
        }
        Ok(files)
    }

    fn persist_records(&self, records: &[SyncRecord]) -> Result<(), std::io::Error> {
        let records_path = if Path::new("records").exists() {
            PathBuf::from("records/sync.jsonl")
        } else if Path::new("../../records").exists() {
            PathBuf::from("../../records/sync.jsonl")
        } else if Path::new("../records").exists() {
            PathBuf::from("../records/sync.jsonl")
        } else {
            PathBuf::from("records/sync.jsonl")
        };
        if let Some(parent) = records_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(records_path)
        {
            for rec in records {
                if let Ok(json) = serde_json::to_string(rec) {
                    let _ = writeln!(f, "{}", json);
                }
            }
        }

        // Persist cache for subsequent CLI invocations
        let cache_path = self.output_dir.join(".rust-swift-sync-cache.json");
        let _ = fs::create_dir_all(&self.output_dir);
        if let Ok(state_json) = serde_json::to_string_pretty(&self.state) {
            let _ = fs::write(cache_path, state_json);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_engine_delta_and_invariant_skipping() {
        let temp_dir = std::env::temp_dir().join("sync_engine_test");
        let src_dir = temp_dir.join("src");
        let out_dir = temp_dir.join("out");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&out_dir).unwrap();

        let file_path = src_dir.join("Compute.rs");
        fs::write(
            &file_path,
            r#"
            pub struct Engine {
                pub power: i32,
            }
            pub fn run(e: Engine) -> i32 {
                e.power.wrapping_add(10)
            }
            "#,
        )
        .unwrap();

        let mut engine = SyncEngine::new(&src_dir, &out_dir);

        // First pass: should detect new file and emit package
        let rec1 = engine.sync_file(&file_path).unwrap();
        assert_eq!(rec1.action, SyncAction::SyncedUpdated);
        assert!(rec1.previous_csg_hash.is_none());
        assert!(!rec1.current_csg_hash.is_empty());
        assert!(out_dir.join("Sources/Compute/Compute.swift").exists());

        // Second pass without changes: should detect unchanged CSG and skip
        let rec2 = engine.sync_file(&file_path).unwrap();
        assert_eq!(rec2.action, SyncAction::SkippedInvariant);
        assert_eq!(rec2.previous_csg_hash, Some(rec1.current_csg_hash.clone()));
        assert_eq!(rec2.current_csg_hash, rec1.current_csg_hash);

        // Third pass with semantic change: should detect CSG delta and update
        fs::write(
            &file_path,
            r#"
            pub struct Engine {
                pub power: i32,
                pub turbo: bool,
            }
            pub fn run(e: Engine) -> i32 {
                e.power.wrapping_add(20)
            }
            "#,
        )
        .unwrap();

        let rec3 = engine.sync_file(&file_path).unwrap();
        assert_eq!(rec3.action, SyncAction::SyncedUpdated);
        assert_ne!(rec3.current_csg_hash, rec1.current_csg_hash);
    }
}
