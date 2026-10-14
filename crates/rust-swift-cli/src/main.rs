//! CLI interface for the Rust-to-Swift compiler.

use clap::{Parser, Subcommand};
use rust_swift_codegen::SwiftGenerator;
use rust_swift_frontend::Frontend;
use rust_swift_qualification::benchmark::BenchmarkRunner;
use rust_swift_qualification::differential::DifferentialRunner;
use rust_swift_qualification::evals::AdversarialRunner;
use rust_swift_qualification::mutation::MutationRunner;
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
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile {
            input,
            output_dir,
            package_name,
            emit_csg,
            emit_ir,
            source_locations,
            spm,
            codable,
            observable,
        } => {
            compile_command(CompileOptions {
                input: &input,
                output_dir: &output_dir,
                package_name: package_name.as_deref(),
                emit_csg,
                emit_ir,
                source_locations,
                spm,
                codable,
                observable,
            })?;
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
        Commands::Proof { input, output } => {
            proof_command(&input, &output)?;
        }
    }

    Ok(())
}

struct CompileOptions<'a> {
    input: &'a Path,
    output_dir: &'a Path,
    package_name: Option<&'a str>,
    emit_csg: bool,
    emit_ir: bool,
    source_locations: bool,
    spm: bool,
    codable: bool,
    observable: bool,
}

fn compile_command(opts: CompileOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let input = opts.input;
    let output_dir = opts.output_dir;
    let package_name = opts.package_name;
    let emit_csg = opts.emit_csg;
    let emit_ir = opts.emit_ir;
    let source_locations = opts.source_locations;
    let spm = opts.spm;
    let codable = opts.codable;
    let observable = opts.observable;

    if !input.exists() {
        eprintln!("Error: Input file does not exist: {}", input.display());
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
        .with_default_observable(observable);
    let module = match frontend.parse_and_lower(&source) {
        Ok(m) => m,
        Err(diags) => {
            eprintln!("Compilation failed with {} diagnostics:", diags.len());
            for d in &diags {
                eprintln!("  {}", d);
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

    println!(
        "Successfully compiled {} to Swift package at {}",
        input.display(),
        output_dir.display()
    );
    println!("  Package name: {}", manifest.package_name);
    println!("  Generated files: {}", manifest.generated_files.join(", "));
    println!("  Source hash: sha256:{}", source_hash);

    Ok(())
}

fn diff_test_command(output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output_dir)?;
    let runner = DifferentialRunner::new(".");

    println!("============================================================");
    println!("Running Rust -> Swift Differential Qualification Suite");
    println!("============================================================");

    const EXPECTED_NEGATIVE_FIXTURES: usize = 10;
    const EXPECTED_TOTAL_REPORTS: usize = 14 + EXPECTED_NEGATIVE_FIXTURES;

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

    // 15. Negative rejections
    println!("\n[15/15] Running Negative Rejection policy test...");
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

fn proof_command(input: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(input)?;
    let file_name = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("input.rs");
    let mut frontend = Frontend::new(file_name);
    let module = match frontend.parse_and_lower(&source) {
        Ok(m) => m,
        Err(diags) => {
            eprintln!("Frontend error: {} diagnostics emitted", diags.len());
            for d in &diags {
                eprintln!("  [{}] {}", d.code.as_str(), d.message);
            }
            std::process::exit(1);
        }
    };

    let receipt = rust_swift_qualification::proof::ProofVerifier::verify(&module, &source);
    let json = serde_json::to_string_pretty(&receipt)?;
    fs::write(output, &json)?;

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

    if !receipt.is_certified {
        std::process::exit(1);
    }

    Ok(())
}
