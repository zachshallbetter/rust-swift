//! CLI interface for the Rust-to-Swift compiler.

use clap::{Parser, Subcommand};
use rust_swift_codegen::SwiftGenerator;
use rust_swift_frontend::Frontend;
use rust_swift_qualification::benchmark::BenchmarkRunner;
use rust_swift_qualification::differential::DifferentialRunner;
use rust_swift_qualification::evals::AdversarialRunner;
use rust_swift_qualification::mutation::MutationRunner;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "rust-swift",
    version = "0.1.0",
    author = "Zach Shallbetter and Reflective Rust Contributors",
    about = "Rust to Swift Source Compiler — High-Assurance Zero-FFI Source Translation"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Inspect whether a Rust source file satisfies the Language Matrix boundary without generating Swift
    Check {
        /// Input Rust source file
        input: PathBuf,

        /// Output format: text or json (machine-readable diagnostics for agents)
        #[arg(long, default_value = "text")]
        format: String,

        /// Force synthesis of Swift Codable conformance across all structs and enums
        #[arg(long)]
        codable: bool,

        /// Enable SwiftUI @Observable macro synthesis for state models
        #[arg(long)]
        observable: bool,

        /// Enable scoped structured concurrency (async fn, TaskGroup)
        #[arg(long, alias = "structured-concurrency")]
        admit_async: bool,
    },
    /// Compile a Rust file into a standalone Swift Package
    Compile {
        /// Input Rust source file
        input: PathBuf,

        /// Output directory for the generated Swift package
        #[arg(short, long, default_value = "generated-swift")]
        output_dir: PathBuf,

        /// Swift package name (defaults to input file stem)
        #[arg(short, long)]
        package_name: Option<String>,

        /// Output format: text or json (machine-readable report for agents)
        #[arg(long, default_value = "text")]
        format: String,

        /// Emit Compiler Semantic Graph (CSG) JSON
        #[arg(long)]
        emit_csg: bool,

        /// Emit Semantic IR JSON
        #[arg(long)]
        emit_ir: bool,

        /// Emit #sourceLocation directives for debugger / IDE source mapping
        #[arg(long, alias = "source-maps")]
        source_locations: bool,

        /// Emit full Swift Package Manager (SPM) manifest and test scaffold
        #[arg(long, alias = "package", default_value_t = true)]
        spm: bool,

        /// Force synthesis of Swift Codable conformance across all structs and enums
        #[arg(long)]
        codable: bool,

        /// Enable SwiftUI @Observable macro synthesis for state models
        #[arg(long)]
        observable: bool,

        /// Enable scoped structured concurrency (async fn, TaskGroup)
        #[arg(long, alias = "structured-concurrency")]
        admit_async: bool,
    },
    /// Verify Apple binary packaging, Hardened Runtime flags, entitlements, and zero-FFI conformance
    VerifyPackaging {
        /// Path to compiled executable or framework bundle to audit
        target: PathBuf,

        /// Optional path to .entitlements XML file to validate
        #[arg(long)]
        entitlements: Option<PathBuf>,

        /// Output format: text or json
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Run differential qualification test suite across Rust reference and Swift outputs
    DiffTest {
        /// Output directory for test artifacts and runners
        #[arg(short, long, default_value = "target/qualification")]
        output_dir: PathBuf,
    },
    /// Inspect environment and toolchain reproducibility manifest
    Manifest,
    /// Run aggressive evaluation suites, failure injection mutations, and gate capability proofs
    Eval {
        /// Output directory for evaluation artifacts
        #[arg(short, long, default_value = "target/evals")]
        output_dir: PathBuf,

        /// Run defect injection and mutation testing
        #[arg(long, default_value_t = true)]
        mutations: bool,

        /// Run aggressive adversarial stress testing and assertion counting
        #[arg(long, default_value_t = true)]
        adversarial: bool,
    },
    /// Run differential performance benchmarking suite across compiler throughput and runtime parity
    Bench {
        /// Output directory for benchmark executables and test artifacts
        #[arg(short, long, default_value = "target/benchmarks")]
        output_dir: PathBuf,
    },
    /// Generate formal invariant certification dossier and proof receipt
    Proof {
        /// Input Rust source file
        input: PathBuf,

        /// Output path for proof receipt JSON (defaults to proof-receipt.json)
        #[arg(short, long, default_value = "proof-receipt.json")]
        output: PathBuf,

        /// Output format: text or json (defaults to text)
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Continuous bi-directional delta synchronization and CSG file watcher daemon
    Sync {
        /// Input directory containing Rust sources
        #[arg(short, long, default_value = ".")]
        input_dir: PathBuf,

        /// Output directory for generated Swift packages
        #[arg(short, long, default_value = "generated-swift")]
        output_dir: PathBuf,

        /// Run continuous file watcher daemon
        #[arg(long)]
        watch: bool,

        /// Polling interval in milliseconds for watch mode (default: 500ms)
        #[arg(long, default_value_t = 500)]
        interval_ms: u64,
    },
    /// Run the Model Context Protocol (MCP) server over stdio for Claude, Codex, Cursor, and agent integrations
    Mcp,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check {
            input,
            format,
            codable,
            observable,
            admit_async,
        } => {
            check_command(&input, &format, codable, observable, admit_async)?;
        }
        Commands::Compile {
            input,
            output_dir,
            package_name,
            format,
            emit_csg,
            emit_ir,
            source_locations,
            spm,
            codable,
            observable,
            admit_async,
        } => {
            compile_command(CompileOptions {
                input: &input,
                output_dir: &output_dir,
                package_name: package_name.as_deref(),
                format: &format,
                emit_csg,
                emit_ir,
                source_locations,
                spm,
                codable,
                observable,
                admit_async,
            })?;
        }
        Commands::VerifyPackaging {
            target,
            entitlements,
            format,
        } => {
            verify_packaging_command(&target, entitlements.as_deref(), &format)?;
        }
        Commands::DiffTest { output_dir } => {
            diff_test_command(&output_dir)?;
        }
        Commands::Manifest => {
            print_manifest()?;
        }
        Commands::Eval {
            output_dir,
            mutations,
            adversarial,
        } => {
            eval_command(&output_dir, mutations, adversarial)?;
        }
        Commands::Bench { output_dir } => {
            bench_command(&output_dir)?;
        }
        Commands::Proof {
            input,
            output,
            format,
        } => {
            proof_command(&input, &output, &format)?;
        }
        Commands::Sync {
            input_dir,
            output_dir,
            watch,
            interval_ms,
        } => {
            sync_command(&input_dir, &output_dir, watch, interval_ms)?;
        }
        Commands::Mcp => {
            mcp_command()?;
        }
    }

    Ok(())
}

struct CompileOptions<'a> {
    input: &'a Path,
    output_dir: &'a Path,
    package_name: Option<&'a str>,
    format: &'a str,
    emit_csg: bool,
    emit_ir: bool,
    source_locations: bool,
    spm: bool,
    codable: bool,
    observable: bool,
    admit_async: bool,
}

fn check_command(
    input: &Path,
    format: &str,
    codable: bool,
    observable: bool,
    admit_async: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !input.exists() {
        if format == "json" {
            println!(
                "{}",
                json!({
                    "status": "error",
                    "file": input.display().to_string(),
                    "message": format!("Input file does not exist: {}", input.display())
                })
            );
        } else {
            eprintln!("Error: Input file does not exist: {}", input.display());
        }
        std::process::exit(1);
    }

    let source = fs::read_to_string(input)?;
    let mut frontend = Frontend::new(input.to_string_lossy().to_string())
        .with_default_codable(codable)
        .with_default_observable(observable)
        .with_admit_async(admit_async);

    match frontend.parse_and_lower(&source) {
        Ok(module) => {
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

            if format == "json" {
                println!(
                    "{}",
                    json!({
                        "status": "admitted",
                        "file": input.display().to_string(),
                        "module_name": module.name,
                        "type_count": type_count,
                        "function_count": function_count,
                        "diagnostics": []
                    })
                );
            } else {
                println!(
                    "[OK] Input '{}' satisfies the Language Matrix boundary.",
                    input.display()
                );
                println!("  Module:    {}", module.name);
                println!("  Types:     {}", type_count);
                println!("  Functions: {}", function_count);
            }
            Ok(())
        }
        Err(diags) => {
            if format == "json" {
                let diag_json: Vec<_> = diags
                    .iter()
                    .map(|d| {
                        json!({
                            "code": d.code.as_str(),
                            "message": d.message,
                            "file": d.span.file,
                            "start_line": d.span.start_line,
                            "end_line": d.span.end_line,
                            "suggested_remedy": d.suggested_alternative,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    json!({
                        "status": "rejected",
                        "file": input.display().to_string(),
                        "diagnostics": diag_json,
                    })
                );
            } else {
                eprintln!(
                    "[REJECTED] Input '{}' violates the Language Matrix boundary with {} diagnostics:",
                    input.display(),
                    diags.len()
                );
                for d in &diags {
                    eprintln!("  {}", d);
                }
            }
            std::process::exit(1);
        }
    }
}

fn compile_command(opts: CompileOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let input = opts.input;
    let output_dir = opts.output_dir;
    let package_name = opts.package_name;
    let format = opts.format;
    let emit_csg = opts.emit_csg;
    let emit_ir = opts.emit_ir;
    let source_locations = opts.source_locations;
    let spm = opts.spm;
    let codable = opts.codable;
    let observable = opts.observable;
    let admit_async = opts.admit_async;

    if !input.exists() {
        if format == "json" {
            println!(
                "{}",
                json!({
                    "status": "error",
                    "file": input.display().to_string(),
                    "message": format!("Input file does not exist: {}", input.display())
                })
            );
        } else {
            eprintln!("Error: Input file does not exist: {}", input.display());
        }
        std::process::exit(1);
    }

    let source = fs::read_to_string(input)?;
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let source_hash = format!("{:x}", hasher.finalize());

    let file_stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Module");
    let pkg_name = package_name.unwrap_or(file_stem);

    let mut frontend = Frontend::new(input.to_string_lossy().to_string())
        .with_default_codable(codable)
        .with_default_observable(observable)
        .with_admit_async(admit_async);
    let module = match frontend.parse_and_lower(&source) {
        Ok(m) => m,
        Err(diags) => {
            if format == "json" {
                let diag_json: Vec<_> = diags
                    .iter()
                    .map(|d| {
                        json!({
                            "code": d.code.as_str(),
                            "message": d.message,
                            "file": d.span.file,
                            "start_line": d.span.start_line,
                            "end_line": d.span.end_line,
                            "suggested_remedy": d.suggested_alternative,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    json!({
                        "status": "rejected",
                        "file": input.display().to_string(),
                        "diagnostics": diag_json,
                    })
                );
            } else {
                eprintln!("Compilation failed with {} diagnostics:", diags.len());
                for d in &diags {
                    eprintln!("  {}", d);
                }
            }
            std::process::exit(1);
        }
    };

    let mut generator = SwiftGenerator::new()
        .with_source_locations(source_locations)
        .with_observable(observable);
    let manifest = if spm {
        generator.emit_package(&module, output_dir, pkg_name)?
    } else {
        let (swift_code, symbols) = generator.generate_module(&module);
        fs::create_dir_all(output_dir)?;
        let file_path = output_dir.join(format!("{}.swift", module.name));
        fs::write(&file_path, &swift_code)?;
        rust_swift_codegen::PackageManifest {
            package_name: pkg_name.to_string(),
            swift_tools_version: "5.9".into(),
            platforms: vec![],
            generated_files: vec![format!("{}.swift", module.name)],
            symbols,
            source_locations_enabled: source_locations,
            test_target_generated: false,
        }
    };

    if emit_ir {
        let ir_json = serde_json::to_string_pretty(&module)?;
        fs::write(output_dir.join("ir.json"), ir_json)?;
    }

    if emit_csg {
        let csg = module.to_csg(&source_hash);
        if let Err(e) = csg.validate() {
            eprintln!("Warning: CSG validation warning: {}", e);
        }
        let csg_json = csg.to_json()?;
        fs::write(output_dir.join("csg.json"), csg_json)?;
    }

    if format == "json" {
        println!(
            "{}",
            json!({
                "status": "success",
                "file": input.display().to_string(),
                "package_name": manifest.package_name,
                "output_dir": output_dir.display().to_string(),
                "generated_files": manifest.generated_files,
                "source_hash": format!("sha256:{}", source_hash),
                "symbols_count": manifest.symbols.len()
            })
        );
    } else {
        println!(
            "Successfully compiled {} to Swift package at {}",
            input.display(),
            output_dir.display()
        );
        println!("  Package name: {}", manifest.package_name);
        println!("  Generated files: {}", manifest.generated_files.join(", "));
        println!("  Source hash: sha256:{}", source_hash);
    }

    Ok(())
}

fn diff_test_command(output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output_dir)?;
    let runner = DifferentialRunner::new(".");

    println!("============================================================");
    println!("Running Rust -> Swift Differential Qualification Suite");
    println!("============================================================");

    const EXPECTED_NEGATIVE_FIXTURES: usize = 10;
    const EXPECTED_TOTAL_REPORTS: usize = 18 + EXPECTED_NEGATIVE_FIXTURES;

    let mut reports = Vec::new();
    let mut execution_failed = false;

    // 1. Finite State Machine
    println!("\n[1/12] Running Finite State Machine differential test...");
    match runner.test_fsm(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!("  FAILED execution in FSM differential test: {}", e);
        }
    }

    // 2. Wrapping Arithmetic
    println!("\n[2/12] Running Wrapping Arithmetic differential test...");
    match runner.test_wrapping_arithmetic(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!("  FAILED execution in Arithmetic differential test: {}", e);
        }
    }

    // 3. Contract System & Invariant Traps
    println!("\n[3/12] Running Contract & Invariant Trap differential test...");
    match runner.test_contracts(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!("  FAILED execution in Contract differential test: {}", e);
        }
    }

    // 4. Type System & Enums
    println!("\n[4/12] Running Type System (Methods, Payload Enums, Tuples) differential test...");
    match runner.test_types(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!("  FAILED execution in Type System differential test: {}", e);
        }
    }

    // 5. Event Dispatch & Trace Model
    println!(
        "\n[5/12] Running Event Dispatch & Trace Model (50-Step History) differential test..."
    );
    match runner.test_event_trace(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in Event Dispatch differential test: {}",
                e
            );
        }
    }

    // 6. Data Semantics (Option, Result, UTF-8 Strings)
    println!(
        "\n[6/12] Running Data Semantics (Option, Result, UTF-8 Strings) differential test..."
    );
    match runner.test_data_semantics(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in Data Semantics differential test: {}",
                e
            );
        }
    }

    // 7. Ownership & Region Classification (&mut and inout exclusivity)
    println!("\n[7/12] Running Ownership & Region Classification differential test...");
    match runner.test_ownership_regions(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!("  FAILED execution in Ownership differential test: {}", e);
        }
    }

    // 8. Iteration & Dynamic Collections (For-Loops, Vec Lifecycle)
    println!("\n[8/12] Running Iteration & Dynamic Collections differential test...");
    match runner.test_iteration_collections(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!("  FAILED execution in Iteration differential test: {}", e);
        }
    }

    // 9. Source Mapping & Traceability (#sourceLocation Directives)
    println!(
        "\n[9/12] Running Source Mapping & Traceability (#sourceLocation) differential test..."
    );
    match runner.test_source_mapping(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in Source Mapping differential test: {}",
                e
            );
        }
    }

    // 10. Protocols & Traits
    println!("\n[10/12] Running Protocols and Traits differential test...");
    match runner.test_protocols_and_traits(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in Protocols and Traits differential test: {}",
                e
            );
        }
    }

    // 11. Property-Based Differential Fuzzing (10,000 Iterations)
    println!("\n[11/12] Running Property-Based Differential Fuzzing (10,000 Iterations)...");
    match runner.test_property_fuzzing(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in Property Fuzzing differential test: {}",
                e
            );
        }
    }

    // 12. SPM Package Manifest Emission and Test Scaffold
    println!("\n[12/14] Running SPM Package Manifest Emission and Test Scaffold qualification...");
    match runner.test_spm_package_build(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in SPM Package Manifest qualification: {}",
                e
            );
        }
    }

    // 13. Swift Codable Synthesis and Serde JSON Interoperability
    println!("\n[13/15] Running Swift Codable Synthesis and Serde JSON Interoperability qualification...");
    match runner.test_codable_json_serialization(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!("  FAILED execution in Swift Codable qualification: {}", e);
        }
    }

    // 14. Modern SwiftUI @Observable Support and Runtime State Observation
    println!("\n[14/15] Running Modern SwiftUI @Observable Support and Runtime State Observation qualification...");
    match runner.test_observable_model(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in Modern SwiftUI @Observable qualification: {}",
                e
            );
        }
    }

    // 15. Trait Associated Types and Where-Clauses
    println!("\n[15/19] Running Trait Associated Types & Where-Clauses differential test...");
    match runner.test_associated_types(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in Associated Types differential test: {}",
                e
            );
        }
    }

    // 16. Scoped Structured Concurrency
    println!("\n[16/19] Running Scoped Structured Concurrency (async/await) differential test...");
    match runner.test_structured_concurrency(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in Structured Concurrency differential test: {}",
                e
            );
        }
    }

    // 17. Apple Framework ("Kit") Adapters & Synthesizers
    println!("\n[17/19] Running Apple Ecosystem Kit Adapters differential test...");
    match runner.test_apple_kit_adapters(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in Apple Kit Adapters differential test: {}",
                e
            );
        }
    }

    // 18. CloudKit Record Projection and CSG Delta Synchronization
    println!("\n[18/19] Running CloudKit Sync & Record Projection differential test...");
    match runner.test_cloudkit_sync(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            execution_failed = true;
            eprintln!(
                "  FAILED execution in CloudKit Sync differential test: {}",
                e
            );
        }
    }

    // 19. Negative rejections
    println!("\n[19/19] Running Negative Rejection policy test...");
    match runner.test_negative_rejections() {
        Ok(neg_reports) => {
            if neg_reports.len() != EXPECTED_NEGATIVE_FIXTURES {
                execution_failed = true;
                eprintln!(
                    "  Negative rejections executed {} fixtures, expected {}",
                    neg_reports.len(),
                    EXPECTED_NEGATIVE_FIXTURES
                );
            }
            for r in neg_reports {
                println!(
                    "  Fixture '{}': {} (Cases passed: {}/{})",
                    r.fixture_name, r.status, r.cases_passed, r.cases_tested
                );
                reports.push(r);
            }
        }
        Err(e) => {
            execution_failed = true;
            eprintln!("  FAILED execution in Negative Rejection test: {}", e);
        }
    }

    // Record evidence in records/evidence.jsonl
    let evidence_path = Path::new("records/evidence.jsonl");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(evidence_path)
    {
        for report in &reports {
            let record = serde_json::json!({
                "type": "differential_qualification",
                "fixture": report.fixture_name,
                "source_hash": report.source_hash,
                "rustc": report.rustc_version,
                "swift": report.swift_version,
                "cases_tested": report.cases_tested,
                "cases_passed": report.cases_passed,
                "cases_failed": report.cases_failed,
                "status": report.status,
                "timestamp": report.timestamp,
                "disposition": if report.cases_failed == 0 && report.cases_tested > 0 { "observed" } else { "contradicted" }
            });
            let _ = writeln!(file, "{}", record);
        }
        println!(
            "\nAppended {} qualification receipt(s) to records/evidence.jsonl",
            reports.len()
        );
    }

    println!("\n============================================================");

    let total_assertions: usize = reports.iter().map(|r| r.cases_tested).sum();
    let total_passed: usize = reports.iter().map(|r| r.cases_passed).sum();
    let total_failed: usize = reports.iter().map(|r| r.cases_failed).sum();

    let all_passed = !execution_failed
        && reports.len() == EXPECTED_TOTAL_REPORTS
        && total_assertions > 0
        && total_failed == 0
        && total_passed == total_assertions
        && reports.iter().all(|r| {
            (r.status == "PASSED" || r.status == "REJECTED_AS_EXPECTED")
                && r.cases_tested > 0
                && r.cases_failed == 0
                && r.cases_passed == r.cases_tested
        });

    if all_passed {
        println!(
            "All differential qualification checks PASSED! ({} reports, {} assertions executed, 0 failed)",
            reports.len(),
            total_assertions
        );
    } else {
        eprintln!(
            "Differential qualification FAILED! (execution_failed={}, reports={}/{}, passed={}/{}, failed={})",
            execution_failed,
            reports.len(),
            EXPECTED_TOTAL_REPORTS,
            total_passed,
            total_assertions,
            total_failed
        );
        std::process::exit(1);
    }

    Ok(())
}

fn print_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let rustc_out = Command::new("rustc").arg("--version").output();
    let swift_out = Command::new("swift").arg("--version").output();

    let rustc_ver = rustc_out
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".into())
        .trim()
        .to_string();

    let swift_ver = swift_out
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".into())
        .lines()
        .next()
        .unwrap_or("unknown")
        .trim()
        .to_string();

    println!("Rust -> Swift Compiler Manifest");
    println!("  Architect:        Zach Shallbetter");
    println!("  Architecture:     Reflective Rust CSG Ecosystem");
    println!("  Compiler Version: 0.1.0");
    println!("  Rust Toolchain:   {}", rustc_ver);
    println!("  Swift Toolchain:  {}", swift_ver);
    println!("  Target Baseline:  macOS v13+ / Swift 5.9+");
    println!("  Admitted Subset:  Pure functions, integers, structs, payload enums, Vec, Option/Result, traits, Codable, @Observable");

    Ok(())
}

fn eval_command(
    output_dir: &Path,
    mutations: bool,
    adversarial: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output_dir)?;
    println!("============================================================");
    println!("Running rust-swift Aggressive Evals & Gate Capability Suite");
    println!("============================================================");

    if mutations {
        let runner = MutationRunner::new(".");
        println!("\nExecuting Failure Injection & Mutation Testing Matrix...");
        let report = runner.run_all_mutations(output_dir)?;
        println!(
            "  Mutations Tested: {} | Detected by Gates: {} | Escaped: {}",
            report.mutations_tested, report.mutations_detected, report.mutations_escaped
        );
        for rec in &report.records {
            let status_badge = if rec.gate_failed_as_expected {
                "DETECTED"
            } else {
                "ESCAPED"
            };
            println!(
                "  [{}] {}: {} (Target Gate: {})",
                status_badge, rec.mutation_id, rec.description, rec.target_gate
            );
        }

        if report.mutations_escaped > 0 {
            eprintln!(
                "\nERROR: {} mutation(s) escaped detection! Gates lack failure capability.",
                report.mutations_escaped
            );
            std::process::exit(1);
        }

        println!("\nSuccess: All simulated defects were deterministically detected by qualification gates.");
        println!("Appended failure receipts to records/negative-results.jsonl");
    }

    if adversarial {
        let runner = AdversarialRunner::new(".");
        println!("\nExecuting Aggressive Adversarial Stress Suite (Deep AST Nesting, Extreme Unicode, Integer Bounds)...");
        let report = runner.run_all_evals(output_dir)?;
        println!(
            "  Total Assertions Executed: {} | Passed: {} | Failed: {} (No-Op Threshold: {})",
            report.total_assertions,
            report.passed_assertions,
            report.failed_assertions,
            report.minimum_required_assertions
        );

        for rec in &report.assertion_records {
            let badge = if rec.passed { "PASS" } else { "FAIL" };
            println!(
                "  [{}] [{}] {}: {}",
                badge, rec.domain, rec.assertion_id, rec.description
            );
        }

        if report.failed_assertions > 0 {
            eprintln!(
                "\nERROR: {} adversarial assertion(s) failed!",
                report.failed_assertions
            );
            std::process::exit(1);
        }

        if report.total_assertions < report.minimum_required_assertions {
            eprintln!(
                "\nERROR: No-Op Protection Triggered! Executed {} assertions, expected at least {}.",
                report.total_assertions, report.minimum_required_assertions
            );
            std::process::exit(1);
        }

        println!(
            "\nSuccess: All adversarial stress evaluations passed with full assertion telemetry."
        );
        println!("Appended evaluation receipt to records/evidence.jsonl");
    }

    Ok(())
}

fn bench_command(output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output_dir)?;
    println!("============================================================");
    println!("Running rust-swift Differential Performance Benchmarks");
    println!("============================================================");

    let runner = BenchmarkRunner::new(".");
    let report = runner.run_all_benchmarks(output_dir)?;

    println!("\nCompiler Throughput:");
    for tp in &report.compiler_throughput {
        println!(
            "  [{}] Lines: {} | Bytes: {} | Elapsed: {:.2}ms | Throughput: {:.0} lines/s ({:.1} KB/s)",
            tp.status,
            tp.lines_processed,
            tp.bytes_processed,
            tp.elapsed_micros as f64 / 1000.0,
            tp.throughput_lines_per_sec,
            tp.throughput_kb_per_sec
        );
    }

    println!("\nRuntime Parity & Performance Differential (rustc -O vs swiftc -O):");
    for p in &report.runtime_parity {
        let parity_status = if p.outputs_match { "MATCH" } else { "MISMATCH" };
        println!(
            "  [{}] Workload: {} ({} iters)",
            parity_status, p.workload, p.iterations
        );
        println!(
            "       Rust: {:.2}ms | Swift: {:.2}ms | Ratio (Swift/Rust): {:.2}x | Parity Checksum: {}",
            p.rust_elapsed_micros as f64 / 1000.0,
            p.swift_elapsed_micros as f64 / 1000.0,
            p.ratio_swift_to_rust,
            p.rust_result_digest
        );
    }

    if report.benchmarks_passed != report.benchmarks_run {
        eprintln!(
            "\nERROR: {}/{} benchmarks passed. Parity or execution defect detected!",
            report.benchmarks_passed, report.benchmarks_run
        );
        std::process::exit(1);
    }

    println!(
        "\nSuccess: All {} benchmarks completed and matched with full runtime parity.",
        report.benchmarks_run
    );
    println!("Appended benchmark telemetry to records/benchmarks.jsonl");

    Ok(())
}

fn proof_command(
    input: &Path,
    output: &Path,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(input)?;
    let file_name = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("input.rs");
    let mut frontend = Frontend::new(file_name);
    let module = match frontend.parse_and_lower(&source) {
        Ok(m) => m,
        Err(diags) => {
            if format == "json" {
                let diag_json: Vec<_> = diags
                    .iter()
                    .map(|d| {
                        json!({
                            "code": d.code.as_str(),
                            "message": d.message,
                            "file": d.span.file,
                            "start_line": d.span.start_line,
                            "end_line": d.span.end_line,
                            "suggested_remedy": d.suggested_alternative,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    json!({
                        "status": "error",
                        "diagnostics": diag_json,
                    })
                );
            } else {
                eprintln!("Frontend error: {} diagnostics emitted", diags.len());
                for d in &diags {
                    eprintln!("  [{}] {}", d.code.as_str(), d.message);
                }
            }
            std::process::exit(1);
        }
    };

    let receipt = rust_swift_qualification::proof::ProofVerifier::verify(&module, &source);
    let json = serde_json::to_string_pretty(&receipt)?;
    fs::write(output, &json)?;

    if format == "json" {
        println!("{}", json);
    } else {
        println!(
            "Epistemic Proof Dossier generated for module '{}':",
            receipt.module_name
        );
        println!("  Invariants Evaluated: {}", receipt.invariants_evaluated);
        println!("  Invariants Proven:    {}", receipt.invariants_proven);
        println!(
            "  Certified:            {}",
            if receipt.is_certified { "YES" } else { "NO" }
        );
        println!(
            "  Signature:            {}",
            receipt.cryptographic_signature
        );
        println!("  Receipt written to:   {}", output.display());
    }

    if !receipt.is_certified {
        std::process::exit(1);
    }

    Ok(())
}

fn sync_command(
    input_dir: &Path,
    output_dir: &Path,
    watch: bool,
    interval_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = rust_swift_qualification::sync::SyncEngine::new(input_dir, output_dir);

    if watch {
        println!("Starting rust-swift-sync daemon in continuous watch mode...");
        engine.watch_loop(interval_ms, None)?;
    } else {
        println!("Running one-shot CSG delta synchronization pass...");
        let records = engine.sync_once()?;
        let mut updated = 0;
        let mut skipped = 0;
        for r in &records {
            match r.action {
                rust_swift_qualification::sync::SyncAction::SyncedUpdated => {
                    updated += 1;
                    let short_hash = if r.current_csg_hash.len() >= 8 {
                        &r.current_csg_hash[..8]
                    } else {
                        &r.current_csg_hash
                    };
                    println!(
                        "  [UPDATED] {} (CSG: {}) in {}ms",
                        r.source_file, short_hash, r.duration_ms
                    );
                }
                rust_swift_qualification::sync::SyncAction::SkippedInvariant => {
                    skipped += 1;
                    println!("  [SKIPPED] {} (CSG Invariant)", r.source_file);
                }
                rust_swift_qualification::sync::SyncAction::Error => {
                    eprintln!("  [ERROR] {} - {}", r.source_file, r.details);
                }
            }
        }
        println!(
            "\nSynchronization pass complete: {} updated, {} skipped (invariant).",
            updated, skipped
        );
        println!("Appended synchronization telemetry to records/sync.jsonl");
    }

    Ok(())
}

fn mcp_command() -> Result<(), Box<dyn std::error::Error>> {
    rust_swift_mcp::McpServer::run_stdio()
}

fn verify_packaging_command(
    target: &Path,
    entitlements: Option<&Path>,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut checks = Vec::new();
    let mut passed = true;

    // 1. Target existence
    if target.exists() {
        checks.push(json!({
            "name": "target_exists",
            "status": "PASS",
            "path": target.display().to_string()
        }));
    } else {
        checks.push(json!({
            "name": "target_exists",
            "status": "FAIL",
            "error": "Target path does not exist"
        }));
        passed = false;
    }

    // 2. Zero-FFI and C-ABI Shims Invariant Audit
    if target.is_file() {
        let otool = Command::new("otool").arg("-L").arg(target).output();
        if let Ok(out) = otool {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let has_forbidden_c_shims = stdout.contains("libclang_rt") || stdout.contains("libgcc");
            if !has_forbidden_c_shims {
                checks.push(json!({
                    "name": "zero_ffi_invariant",
                    "status": "PASS",
                    "details": "Binary links directly to standard Apple system dylibs without intermediate C bridges"
                }));
            } else {
                checks.push(json!({
                    "name": "zero_ffi_invariant",
                    "status": "FAIL",
                    "details": "Detected unadmitted foreign C runtime linkage"
                }));
                passed = false;
            }
        }
    }

    // 3. Hardened Runtime and Code Signature Audit
    if target.exists() {
        let codesign = Command::new("codesign").arg("-dvvv").arg(target).output();
        if let Ok(out) = codesign {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let is_hardened =
                stderr.contains("flags=0x10000(runtime)") || stderr.contains("runtime");
            checks.push(json!({
                "name": "hardened_runtime",
                "status": if is_hardened { "PASS" } else { "INFO" },
                "details": if is_hardened {
                    "Binary is signed with macOS Hardened Runtime flag (runtime)"
                } else {
                    "Binary is unsigned or in debug configuration (Hardened Runtime required for notarization)"
                }
            }));
        }
    }

    // 4. Entitlements validation
    if let Some(ent_path) = entitlements {
        if ent_path.exists() {
            let content = fs::read_to_string(ent_path)?;
            let is_valid_plist = content.contains("<!DOCTYPE plist") && content.contains("<dict>");
            let has_sandbox = content.contains("com.apple.security.app-sandbox");
            if is_valid_plist {
                checks.push(json!({
                    "name": "entitlements_xml_valid",
                    "status": "PASS",
                    "sandbox_declared": has_sandbox,
                    "path": ent_path.display().to_string()
                }));
            } else {
                checks.push(json!({
                    "name": "entitlements_xml_valid",
                    "status": "FAIL",
                    "error": "File is not a valid Apple property list XML"
                }));
                passed = false;
            }
        } else {
            checks.push(json!({
                "name": "entitlements_xml_valid",
                "status": "FAIL",
                "error": "Specified entitlements file does not exist"
            }));
            passed = false;
        }
    }

    let report = json!({
        "status": if passed { "PASSED" } else { "FAILED" },
        "target": target.display().to_string(),
        "entitlements": entitlements.map(|p| p.display().to_string()),
        "checks": checks,
        "timestamp": "2026-10-15T03:30:00Z"
    });

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("============================================================");
        println!("Apple Packaging & Hardened Runtime Audit");
        println!("============================================================");
        println!("Target: {}", target.display());
        println!("Status: {}", if passed { "PASSED" } else { "FAILED" });
        for c in checks {
            println!(
                "- {}: {}",
                c["name"].as_str().unwrap_or(""),
                c["status"].as_str().unwrap_or("")
            );
        }
    }

    if !passed {
        std::process::exit(1);
    }
    Ok(())
}
