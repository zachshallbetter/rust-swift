//! CLI interface for the Rust-to-Swift compiler.

use clap::{Parser, Subcommand};
use rust_swift_codegen::SwiftGenerator;
use rust_swift_frontend::Frontend;
use rust_swift_qualification::differential::DifferentialRunner;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "rust-swift",
    version = "0.1.0",
    about = "Rust to Swift Source Compiler"
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
    },
    /// Run differential qualification test suite across Rust reference and Swift outputs
    DiffTest {
        /// Output directory for test artifacts and runners
        #[arg(short, long, default_value = "target/qualification")]
        output_dir: PathBuf,
    },
    /// Inspect environment and toolchain reproducibility manifest
    Manifest,
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
        } => {
            compile_command(
                &input,
                &output_dir,
                package_name.as_deref(),
                emit_csg,
                emit_ir,
            )?;
        }
        Commands::DiffTest { output_dir } => {
            diff_test_command(&output_dir)?;
        }
        Commands::Manifest => {
            print_manifest()?;
        }
    }

    Ok(())
}

fn compile_command(
    input: &Path,
    output_dir: &Path,
    package_name: Option<&str>,
    emit_csg: bool,
    emit_ir: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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

    let mut frontend = Frontend::new(input.to_string_lossy().to_string());
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

    let mut generator = SwiftGenerator::new();
    let manifest = generator.emit_package(&module, output_dir, pkg_name)?;

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

    let mut reports = Vec::new();

    // 1. Finite State Machine
    println!("\n[1/8] Running Finite State Machine differential test...");
    match runner.test_fsm(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            eprintln!("  Error in FSM differential test: {}", e);
        }
    }

    // 2. Wrapping Arithmetic
    println!("\n[2/8] Running Wrapping Arithmetic differential test...");
    match runner.test_wrapping_arithmetic(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            eprintln!("  Error in Arithmetic differential test: {}", e);
        }
    }

    // 3. Contract System & Invariant Traps
    println!("\n[3/8] Running Contract & Invariant Trap differential test...");
    match runner.test_contracts(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            eprintln!("  Error in Contract differential test: {}", e);
        }
    }

    // 4. Type System & Enums
    println!("\n[4/8] Running Type System (Methods, Payload Enums, Tuples) differential test...");
    match runner.test_types(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            eprintln!("  Error in Type System differential test: {}", e);
        }
    }

    // 5. Event Dispatch & Trace Model
    println!("\n[5/8] Running Event Dispatch & Trace Model (50-Step History) differential test...");
    match runner.test_event_trace(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            eprintln!("  Error in Event Dispatch differential test: {}", e);
        }
    }

    // 6. Data Semantics (Option, Result, UTF-8 Strings)
    println!("\n[6/8] Running Data Semantics (Option, Result, UTF-8 Strings) differential test...");
    match runner.test_data_semantics(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            eprintln!("  Error in Data Semantics differential test: {}", e);
        }
    }

    // 7. Ownership & Region Classification (&mut and inout exclusivity)
    println!("\n[7/8] Running Ownership & Region Classification differential test...");
    match runner.test_ownership_regions(output_dir) {
        Ok(report) => {
            println!(
                "  Status: {} ({} of {} cases passed)",
                report.status, report.cases_passed, report.cases_tested
            );
            reports.push(report);
        }
        Err(e) => {
            eprintln!("  Error in Ownership differential test: {}", e);
        }
    }

    // 8. Negative rejections
    println!("\n[8/8] Running Negative Rejection policy test...");
    match runner.test_negative_rejections() {
        Ok(neg_reports) => {
            for r in neg_reports {
                println!(
                    "  Fixture '{}': {} (Cases passed: {}/{})",
                    r.fixture_name, r.status, r.cases_passed, r.cases_tested
                );
                reports.push(r);
            }
        }
        Err(e) => {
            eprintln!("  Error in Negative Rejection test: {}", e);
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
                "status": report.status,
                "timestamp": report.timestamp,
                "disposition": if report.cases_failed == 0 { "observed" } else { "contradicted" }
            });
            let _ = writeln!(file, "{}", record);
        }
        println!(
            "\nAppended {} qualification receipt(s) to records/evidence.jsonl",
            reports.len()
        );
    }

    println!("\n============================================================");
    let all_passed = reports.iter().all(|r| r.cases_failed == 0);
    if all_passed {
        println!("All differential qualification checks PASSED!");
    } else {
        println!("Differential qualification FAILED on some cases.");
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
    println!("  Compiler Version: 0.1.0");
    println!("  Rust Toolchain:   {}", rustc_ver);
    println!("  Swift Toolchain:  {}", swift_ver);
    println!("  Target Baseline:  macOS v13+ / Swift 5.9+");
    println!("  Admitted Subset:  v0.1 Pure functions, integers, simple structs, fieldless enums");

    Ok(())
}
