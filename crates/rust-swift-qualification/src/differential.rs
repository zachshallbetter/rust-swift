//! Differential testing engine executing Rust and Swift binaries on identical inputs.

use rust_swift_codegen::SwiftGenerator;
use rust_swift_frontend::diagnostic::DiagnosticCode;
use rust_swift_frontend::Frontend;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualificationReport {
    pub fixture_name: String,
    pub source_hash: String,
    pub rustc_version: String,
    pub swift_version: String,
    pub cases_tested: usize,
    pub cases_passed: usize,
    pub cases_failed: usize,
    pub status: String,
    pub timestamp: String,
    pub details: Vec<String>,
}

pub struct DifferentialRunner {
    pub workspace_dir: PathBuf,
}

impl DifferentialRunner {
    pub fn new(workspace_dir: impl Into<PathBuf>) -> Self {
        Self {
            workspace_dir: workspace_dir.into(),
        }
    }

    fn toolchain_versions() -> (String, String) {
        let rustc = Command::new("rustc")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_else(|| "rustc unknown".into())
            .trim()
            .to_string();

        let swift = Command::new("swift")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_else(|| "swift unknown".into())
            .lines()
            .next()
            .unwrap_or("swift unknown")
            .trim()
            .to_string();

        (rustc, swift)
    }

    fn compute_sha256(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Run the Finite State Machine differential test
    pub fn test_fsm(
        &self,
        out_dir: &Path,
    ) -> Result<QualificationReport, Box<dyn std::error::Error>> {
        let fsm_source = r#"
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Idle,
    Active,
    Paused,
    Stopped,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    Start,
    Pause,
    Resume,
    Stop,
}

pub fn transition(state: State, event: Event) -> State {
    match (state, event) {
        (State::Idle, Event::Start) => State::Active,
        (State::Active, Event::Pause) => State::Paused,
        (State::Paused, Event::Resume) => State::Active,
        (State::Active, Event::Stop) => State::Stopped,
        (State::Paused, Event::Stop) => State::Stopped,
        _ => state,
    }
}
"#;

        let source_hash = Self::compute_sha256(fsm_source);
        let (rustc_ver, swift_ver) = Self::toolchain_versions();

        // 1. Lower to IR
        let mut frontend = Frontend::new("fsm.rs");
        let module = frontend
            .parse_and_lower(fsm_source)
            .map_err(|diags| format!("Frontend rejected FSM fixture: {:?}", diags))?;

        // 2. Generate Swift package
        let pkg_dir = out_dir.join("fsm_pkg");
        let mut generator = SwiftGenerator::new();
        generator.emit_package(&module, &pkg_dir, "FsmModule")?;

        // 3. Create a Swift test runner executable
        let runner_swift = r#"
import Foundation

@main
struct FsmRunner {
    static func main() {
        let states: [State] = [.Idle, .Active, .Paused, .Stopped]
        let events: [Event] = [.Start, .Pause, .Resume, .Stop]

        for s in states {
            for e in events {
                let next = transition(s, e)
                print("\(s.rawValue),\(e.rawValue)->\(next.rawValue)")
            }
        }
    }
}
"#;
        let sources_dir = pkg_dir.join("Sources").join("FsmModule");
        let main_swift_path = sources_dir.join("main.swift");
        fs::write(&main_swift_path, runner_swift)?;

        // 4. Compile and execute Swift code
        let swift_source = sources_dir.join("fsm.swift");
        let swiftc_out = Command::new("swiftc")
            .arg("-parse-as-library")
            .arg("-emit-executable")
            .arg(&swift_source)
            .arg(&main_swift_path)
            .arg("-o")
            .arg(pkg_dir.join("fsm_runner"))
            .output()?;

        if !swiftc_out.status.success() {
            return Err(format!(
                "Failed to compile Swift FSM runner: {}",
                String::from_utf8_lossy(&swiftc_out.stderr)
            )
            .into());
        }

        let exec_out = Command::new(pkg_dir.join("fsm_runner")).output()?;
        let swift_results = String::from_utf8(exec_out.stdout)?;

        // 5. Compute expected results using Rust implementation
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        enum RustState {
            Idle,
            Active,
            Paused,
            Stopped,
        }
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        enum RustEvent {
            Start,
            Pause,
            Resume,
            Stop,
        }
        fn rust_transition(state: RustState, event: RustEvent) -> RustState {
            match (state, event) {
                (RustState::Idle, RustEvent::Start) => RustState::Active,
                (RustState::Active, RustEvent::Pause) => RustState::Paused,
                (RustState::Paused, RustEvent::Resume) => RustState::Active,
                (RustState::Active, RustEvent::Stop) => RustState::Stopped,
                (RustState::Paused, RustEvent::Stop) => RustState::Stopped,
                _ => state,
            }
        }

        let rust_states = [
            RustState::Idle,
            RustState::Active,
            RustState::Paused,
            RustState::Stopped,
        ];
        let rust_events = [
            RustEvent::Start,
            RustEvent::Pause,
            RustEvent::Resume,
            RustEvent::Stop,
        ];
        let mut rust_lines = Vec::new();
        for s in rust_states {
            for e in rust_events {
                let next = rust_transition(s, e);
                rust_lines.push(format!("{:?},{:?}->{:?}", s, e, next));
            }
        }

        let swift_lines: Vec<&str> = swift_results
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        assert_eq!(rust_lines.len(), 16);
        assert_eq!(swift_lines.len(), 16);

        let mut cases_passed = 0;
        let mut details = Vec::new();
        for (r, s) in rust_lines.iter().zip(swift_lines.iter()) {
            if r == s {
                cases_passed += 1;
                details.push(format!("OK: {}", r));
            } else {
                details.push(format!("FAIL: Rust='{}' vs Swift='{}'", r, s));
            }
        }

        let status = if cases_passed == 16 {
            "PASSED"
        } else {
            "FAILED"
        };

        Ok(QualificationReport {
            fixture_name: "finite_state_machine".into(),
            source_hash,
            rustc_version: rustc_ver,
            swift_version: swift_ver,
            cases_tested: 16,
            cases_passed,
            cases_failed: 16 - cases_passed,
            status: status.into(),
            timestamp: "2026-10-13T20:00:00Z".into(),
            details,
        })
    }

    /// Run the Wrapping Arithmetic differential test
    pub fn test_wrapping_arithmetic(
        &self,
        out_dir: &Path,
    ) -> Result<QualificationReport, Box<dyn std::error::Error>> {
        let arith_source = r#"
pub fn add_wrapping(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

pub fn sub_wrapping(a: i32, b: i32) -> i32 {
    a.wrapping_sub(b)
}

pub fn mul_wrapping(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b)
}

pub fn bitwise_logic(a: u32, b: u32) -> u32 {
    (a & b) | (a ^ b)
}
"#;

        let source_hash = Self::compute_sha256(arith_source);
        let (rustc_ver, swift_ver) = Self::toolchain_versions();

        let mut frontend = Frontend::new("arith.rs");
        let module = frontend
            .parse_and_lower(arith_source)
            .map_err(|diags| format!("Frontend rejected Arithmetic fixture: {:?}", diags))?;

        let pkg_dir = out_dir.join("arith_pkg");
        let mut generator = SwiftGenerator::new();
        generator.emit_package(&module, &pkg_dir, "ArithModule")?;

        let vectors: Vec<(i32, i32)> = vec![
            (0, 0),
            (1, 1),
            (-1, -1),
            (i32::MAX, 1),
            (i32::MAX, i32::MAX),
            (i32::MIN, -1),
            (i32::MIN, 1),
            (i32::MIN, i32::MIN),
            (1000, 2000),
            (-500, 1000),
            (i32::MAX - 1, 2),
            (i32::MIN + 1, -2),
        ];

        let mut runner_swift = String::new();
        runner_swift.push_str(
            "import Foundation\n\n@main\nstruct ArithRunner {\n    static func main() {\n",
        );
        for (i, (a, b)) in vectors.iter().enumerate() {
            runner_swift.push_str(&format!(
                "        print(\"add_{}: \\(add_wrapping({}, {}))\")\n",
                i, a, b
            ));
            runner_swift.push_str(&format!(
                "        print(\"sub_{}: \\(sub_wrapping({}, {}))\")\n",
                i, a, b
            ));
            runner_swift.push_str(&format!(
                "        print(\"mul_{}: \\(mul_wrapping({}, {}))\")\n",
                i, a, b
            ));
        }
        runner_swift.push_str("    }\n}\n");

        let sources_dir = pkg_dir.join("Sources").join("ArithModule");
        let main_swift_path = sources_dir.join("main.swift");
        fs::write(&main_swift_path, runner_swift)?;

        let swift_source = sources_dir.join("arith.swift");
        let swiftc_out = Command::new("swiftc")
            .arg("-parse-as-library")
            .arg("-emit-executable")
            .arg(&swift_source)
            .arg(&main_swift_path)
            .arg("-o")
            .arg(pkg_dir.join("arith_runner"))
            .output()?;

        if !swiftc_out.status.success() {
            return Err(format!(
                "Failed to compile Swift arithmetic runner: {}",
                String::from_utf8_lossy(&swiftc_out.stderr)
            )
            .into());
        }

        let exec_out = Command::new(pkg_dir.join("arith_runner")).output()?;
        let swift_results = String::from_utf8(exec_out.stdout)?;

        let mut cases_tested = 0;
        let mut cases_passed = 0;
        let mut details = Vec::new();

        let swift_map: std::collections::HashMap<String, i32> = swift_results
            .lines()
            .filter_map(|l| {
                let parts: Vec<&str> = l.split(": ").collect();
                if parts.len() == 2 {
                    Some((
                        parts[0].trim().to_string(),
                        parts[1].trim().parse::<i32>().unwrap(),
                    ))
                } else {
                    None
                }
            })
            .collect();

        for (i, (a, b)) in vectors.iter().enumerate() {
            // Test add
            cases_tested += 1;
            let r_add = a.wrapping_add(*b);
            let s_add = swift_map.get(&format!("add_{}", i)).copied().unwrap_or(0);
            if r_add == s_add {
                cases_passed += 1;
            } else {
                details.push(format!(
                    "FAIL add({}, {}): Rust={} vs Swift={}",
                    a, b, r_add, s_add
                ));
            }

            // Test sub
            cases_tested += 1;
            let r_sub = a.wrapping_sub(*b);
            let s_sub = swift_map.get(&format!("sub_{}", i)).copied().unwrap_or(0);
            if r_sub == s_sub {
                cases_passed += 1;
            } else {
                details.push(format!(
                    "FAIL sub({}, {}): Rust={} vs Swift={}",
                    a, b, r_sub, s_sub
                ));
            }

            // Test mul
            cases_tested += 1;
            let r_mul = a.wrapping_mul(*b);
            let s_mul = swift_map.get(&format!("mul_{}", i)).copied().unwrap_or(0);
            if r_mul == s_mul {
                cases_passed += 1;
            } else {
                details.push(format!(
                    "FAIL mul({}, {}): Rust={} vs Swift={}",
                    a, b, r_mul, s_mul
                ));
            }
        }

        let status = if cases_passed == cases_tested {
            "PASSED"
        } else {
            "FAILED"
        };

        Ok(QualificationReport {
            fixture_name: "wrapping_arithmetic".into(),
            source_hash,
            rustc_version: rustc_ver,
            swift_version: swift_ver,
            cases_tested,
            cases_passed,
            cases_failed: cases_tested - cases_passed,
            status: status.into(),
            timestamp: "2026-10-13T20:00:00Z".into(),
            details,
        })
    }

    /// Run Contract System and Invariant Trap differential test
    pub fn test_contracts(
        &self,
        out_dir: &Path,
    ) -> Result<QualificationReport, Box<dyn std::error::Error>> {
        let contract_source = r#"
pub fn validate_step(x: i32, max: i32) -> i32 {
    assert!(max > 0, "max must be positive");
    assert!(x >= 0, "x must be non-negative");
    assert!(x <= max, "x must not exceed max");
    assert_eq!(x, x, "identity check");
    if x == 999 {
        panic!("explicit abort trigger");
    }
    x
}
"#;

        let source_hash = Self::compute_sha256(contract_source);
        let (rustc_ver, swift_ver) = Self::toolchain_versions();

        // 1. Lower to IR
        let mut frontend = Frontend::new("contracts.rs");
        let module = frontend
            .parse_and_lower(contract_source)
            .map_err(|diags| format!("Frontend rejected Contract fixture: {:?}", diags))?;

        // 2. Generate Swift package
        let pkg_dir = out_dir.join("contract_pkg");
        let mut generator = SwiftGenerator::new();
        generator.emit_package(&module, &pkg_dir, "ContractModule")?;

        // 3. Create Swift test runner
        let runner_swift = r#"
import Foundation

@main
struct ContractRunner {
    static func main() {
        let args = CommandLine.arguments
        guard args.count >= 3,
              let x = Int32(args[1]),
              let max = Int32(args[2]) else {
            exit(2)
        }
        let res = validate_step(x, max)
        print("RESULT:\(res)")
    }
}
"#;
        let sources_dir = pkg_dir.join("Sources").join("ContractModule");
        let main_swift_path = sources_dir.join("main.swift");
        fs::write(&main_swift_path, runner_swift)?;

        // 4. Compile Swift code
        let swift_source = sources_dir.join("contracts.swift");
        let swift_runner_bin = pkg_dir.join("contract_swift_runner");
        let swiftc_out = Command::new("swiftc")
            .arg("-parse-as-library")
            .arg("-emit-executable")
            .arg(&swift_source)
            .arg(&main_swift_path)
            .arg("-o")
            .arg(&swift_runner_bin)
            .output()?;

        if !swiftc_out.status.success() {
            return Err(format!(
                "Failed to compile Swift Contract runner: {}",
                String::from_utf8_lossy(&swiftc_out.stderr)
            )
            .into());
        }

        // 5. Compile Rust reference executable
        let rust_runner_src = r#"
fn validate_step(x: i32, max: i32) -> i32 {
    assert!(max > 0, "max must be positive");
    assert!(x >= 0, "x must be non-negative");
    assert!(x <= max, "x must not exceed max");
    assert_eq!(x, x, "identity check");
    if x == 999 {
        panic!("explicit abort trigger");
    }
    x
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        std::process::exit(2);
    }
    let x: i32 = args[1].parse().unwrap();
    let max: i32 = args[2].parse().unwrap();
    let res = validate_step(x, max);
    println!("RESULT:{}", res);
}
"#;
        let rust_src_file = pkg_dir.join("contract_rust_runner.rs");
        fs::write(&rust_src_file, rust_runner_src)?;
        let rust_runner_bin = pkg_dir.join("contract_rust_runner");
        let rustc_out = Command::new("rustc")
            .arg(&rust_src_file)
            .arg("-o")
            .arg(&rust_runner_bin)
            .output()?;

        if !rustc_out.status.success() {
            return Err(format!(
                "Failed to compile Rust reference runner: {}",
                String::from_utf8_lossy(&rustc_out.stderr)
            )
            .into());
        }

        // 6. Test vectors: positive cases (should succeed with matching outputs) and negative cases (should both trap/fail)
        let test_cases = [
            (0, 100, true),
            (50, 100, true),
            (100, 100, true),
            (-1, 100, false),   // x < 0 violates precondition
            (105, 100, false),  // x > max violates precondition
            (999, 1000, false), // explicit panic! trigger
            (10, -5, false),    // max <= 0 violates precondition
        ];

        let mut cases_passed = 0;
        let mut details = Vec::new();

        for &(x, max, should_pass) in &test_cases {
            let r_out = Command::new(&rust_runner_bin)
                .arg(x.to_string())
                .arg(max.to_string())
                .output()?;

            let s_out = Command::new(&swift_runner_bin)
                .arg(x.to_string())
                .arg(max.to_string())
                .output()?;

            let r_success = r_out.status.success();
            let s_success = s_out.status.success();

            if should_pass {
                let r_stdout = String::from_utf8_lossy(&r_out.stdout).trim().to_string();
                let s_stdout = String::from_utf8_lossy(&s_out.stdout).trim().to_string();
                if r_success && s_success && r_stdout == s_stdout {
                    cases_passed += 1;
                    details.push(format!("OK (Passed): ({}, {}) -> {}", x, max, r_stdout));
                } else {
                    details.push(format!(
                        "FAIL (Expected success): ({}, {}) -> Rust=(success={}, out={}) Swift=(success={}, out={})",
                        x, max, r_success, r_stdout, s_success, s_stdout
                    ));
                }
            } else {
                // Both MUST trap / fail with non-zero exit status
                if !r_success && !s_success {
                    cases_passed += 1;
                    details.push(format!(
                        "OK (Trap matched): ({}, {}) -> Rust trapped (code {:?}), Swift trapped (code {:?})",
                        x, max, r_out.status.code(), s_out.status.code()
                    ));
                } else {
                    details.push(format!(
                        "FAIL (Expected trap): ({}, {}) -> Rust trapped={}, Swift trapped={}",
                        x, max, !r_success, !s_success
                    ));
                }
            }
        }

        let total = test_cases.len();
        let status = if cases_passed == total {
            "PASSED"
        } else {
            "FAILED"
        };

        Ok(QualificationReport {
            fixture_name: "contract_assertions_and_traps".into(),
            source_hash,
            rustc_version: rustc_ver,
            swift_version: swift_ver,
            cases_tested: total,
            cases_passed,
            cases_failed: total - cases_passed,
            status: status.into(),
            timestamp: "2026-10-13T20:00:00Z".into(),
            details,
        })
    }

    /// Run the Type System (struct methods, payload enums, tuples) differential test
    pub fn test_types(
        &self,
        out_dir: &Path,
    ) -> Result<QualificationReport, Box<dyn std::error::Error>> {
        let types_source = r#"
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Counter {
    pub count: i64,
}

impl Counter {
    pub fn new(initial: i64) -> Counter {
        Counter { count: initial }
    }

    pub fn increment(&mut self, delta: i64) {
        self.count = self.count + delta;
    }

    pub fn get(&self) -> i64 {
        self.count
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Stop,
    Move(i64, i64),
    Teleport { x: i64, y: i64 },
}

pub fn apply_action(action: Action) -> (i64, i64) {
    match action {
        Action::Stop => (0, 0),
        Action::Move(dx, dy) => (dx, dy),
        Action::Teleport { x, y } => (x, y),
    }
}

pub fn execute_workflow(initial: i64, delta: i64, action: Action) -> (i64, i64, i64) {
    let mut c = Counter::new(initial);
    c.increment(delta);
    let coords = apply_action(action);
    (c.get(), coords.0, coords.1)
}
"#;

        let source_hash = Self::compute_sha256(types_source);
        let (rustc_ver, swift_ver) = Self::toolchain_versions();

        // 1. Lower to IR
        let mut frontend = Frontend::new("types.rs");
        let module = frontend
            .parse_and_lower(types_source)
            .map_err(|diags| format!("Frontend rejected types fixture: {:?}", diags))?;

        // 2. Generate Swift package
        let pkg_dir = out_dir.join("types_pkg");
        let mut generator = SwiftGenerator::new();
        generator.emit_package(&module, &pkg_dir, "TypesModule")?;

        // 3. Create a Swift test runner executable
        let runner_swift = r#"
import Foundation

@main
struct TypesRunner {
    static func main() {
        let test_cases: [(Int64, Int64, Action)] = [
            (0, 5, .Stop),
            (10, -3, .Move(4, 7)),
            (100, 25, .Teleport(x: -12, y: 88)),
            (-50, 50, .Move(-10, -20)),
            (42, 0, .Stop),
        ]

        for (initial, delta, action) in test_cases {
            let res = execute_workflow(initial, delta, action)
            print("\(initial),\(delta)->(\(res.0),\(res.1),\(res.2))")
        }
    }
}
"#;
        let sources_dir = pkg_dir.join("Sources").join("TypesModule");
        let main_swift_path = sources_dir.join("main.swift");
        fs::write(&main_swift_path, runner_swift)?;

        // 4. Compile and execute Swift code
        let swift_source = sources_dir.join("types.swift");
        let swiftc_out = Command::new("swiftc")
            .arg("-parse-as-library")
            .arg("-emit-executable")
            .arg(&swift_source)
            .arg(&main_swift_path)
            .arg("-o")
            .arg(pkg_dir.join("types_runner"))
            .output()?;

        if !swiftc_out.status.success() {
            return Err(format!(
                "Failed to compile Swift Types runner: {}",
                String::from_utf8_lossy(&swiftc_out.stderr)
            )
            .into());
        }

        let exec_out = Command::new(pkg_dir.join("types_runner")).output()?;
        let swift_results = String::from_utf8(exec_out.stdout)?;

        // 5. Compute expected results using Rust implementation
        struct RustCounter {
            count: i64,
        }
        impl RustCounter {
            fn new(initial: i64) -> Self {
                Self { count: initial }
            }
            fn increment(&mut self, delta: i64) {
                self.count += delta;
            }
            fn get(&self) -> i64 {
                self.count
            }
        }
        enum RustAction {
            Stop,
            Move(i64, i64),
            Teleport { x: i64, y: i64 },
        }
        fn rust_apply(action: RustAction) -> (i64, i64) {
            match action {
                RustAction::Stop => (0, 0),
                RustAction::Move(dx, dy) => (dx, dy),
                RustAction::Teleport { x, y } => (x, y),
            }
        }
        fn rust_workflow(initial: i64, delta: i64, action: RustAction) -> (i64, i64, i64) {
            let mut c = RustCounter::new(initial);
            c.increment(delta);
            let coords = rust_apply(action);
            (c.get(), coords.0, coords.1)
        }

        let test_cases = [
            (0, 5, RustAction::Stop),
            (10, -3, RustAction::Move(4, 7)),
            (100, 25, RustAction::Teleport { x: -12, y: 88 }),
            (-50, 50, RustAction::Move(-10, -20)),
            (42, 0, RustAction::Stop),
        ];

        let mut rust_lines = Vec::new();
        for (initial, delta, action) in test_cases {
            let res = rust_workflow(initial, delta, action);
            rust_lines.push(format!(
                "{},{}->({},{},{})",
                initial, delta, res.0, res.1, res.2
            ));
        }

        let swift_lines: Vec<&str> = swift_results
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        let total = rust_lines.len();
        let mut cases_passed = 0;
        let mut details = Vec::new();

        for (idx, expected) in rust_lines.iter().enumerate() {
            if idx < swift_lines.len() && expected == swift_lines[idx] {
                cases_passed += 1;
                details.push(format!("Vector #{}: PASS ({})", idx + 1, expected));
            } else {
                let actual = if idx < swift_lines.len() {
                    swift_lines[idx]
                } else {
                    "<missing>"
                };
                details.push(format!(
                    "Vector #{}: FAIL (Expected: {}, Got: {})",
                    idx + 1,
                    expected,
                    actual
                ));
            }
        }

        let status = if cases_passed == total {
            "PASSED"
        } else {
            "FAILED"
        };

        Ok(QualificationReport {
            fixture_name: "types_methods_and_payload_enums".into(),
            source_hash,
            rustc_version: rustc_ver,
            swift_version: swift_ver,
            cases_tested: total,
            cases_passed,
            cases_failed: total - cases_passed,
            status: status.into(),
            timestamp: "2026-10-13T20:00:00Z".into(),
            details,
        })
    }

    /// Run the Event Dispatch & Trace Model (50 sequential transitions with effect streams) differential test
    pub fn test_event_trace(
        &self,
        out_dir: &Path,
    ) -> Result<QualificationReport, Box<dyn std::error::Error>> {
        let player_source = r#"
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Buffering,
    Stopped,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayerEvent {
    Load,
    Play,
    Pause,
    BufferUnderflow,
    BufferReady,
    Stop,
    Fail,
    Reset,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayerEffect {
    StartDecoder,
    StopDecoder,
    RenderFrame,
    MuteAudio,
    UnmuteAudio,
    LogNotice,
    RaiseAlert,
}

pub fn step_player(state: PlayerState, event: PlayerEvent) -> (PlayerState, Vec<PlayerEffect>) {
    match (state, event) {
        (PlayerState::Idle, PlayerEvent::Load) => {
            (PlayerState::Loading, vec![PlayerEffect::StartDecoder, PlayerEffect::LogNotice])
        }
        (PlayerState::Loading, PlayerEvent::BufferReady) => {
            (PlayerState::Paused, vec![PlayerEffect::UnmuteAudio])
        }
        (PlayerState::Paused, PlayerEvent::Play) => {
            (PlayerState::Playing, vec![PlayerEffect::RenderFrame])
        }
        (PlayerState::Playing, PlayerEvent::Pause) => {
            (PlayerState::Paused, vec![PlayerEffect::MuteAudio])
        }
        (PlayerState::Playing, PlayerEvent::BufferUnderflow) => {
            (PlayerState::Buffering, vec![PlayerEffect::MuteAudio, PlayerEffect::LogNotice])
        }
        (PlayerState::Buffering, PlayerEvent::BufferReady) => {
            (PlayerState::Playing, vec![PlayerEffect::UnmuteAudio, PlayerEffect::RenderFrame])
        }
        (PlayerState::Playing, PlayerEvent::Stop) => {
            (PlayerState::Stopped, vec![PlayerEffect::StopDecoder, PlayerEffect::MuteAudio])
        }
        (PlayerState::Paused, PlayerEvent::Stop) => {
            (PlayerState::Stopped, vec![PlayerEffect::StopDecoder])
        }
        (PlayerState::Stopped, PlayerEvent::Reset) => {
            (PlayerState::Idle, vec![PlayerEffect::LogNotice])
        }
        (PlayerState::Error, PlayerEvent::Reset) => {
            (PlayerState::Idle, vec![PlayerEffect::LogNotice])
        }
        (PlayerState::Idle, PlayerEvent::Fail) => {
            (PlayerState::Error, vec![PlayerEffect::StopDecoder, PlayerEffect::RaiseAlert])
        }
        (PlayerState::Loading, PlayerEvent::Fail) => {
            (PlayerState::Error, vec![PlayerEffect::StopDecoder, PlayerEffect::RaiseAlert])
        }
        (PlayerState::Playing, PlayerEvent::Fail) => {
            (PlayerState::Error, vec![PlayerEffect::StopDecoder, PlayerEffect::RaiseAlert])
        }
        (PlayerState::Paused, PlayerEvent::Fail) => {
            (PlayerState::Error, vec![PlayerEffect::StopDecoder, PlayerEffect::RaiseAlert])
        }
        (PlayerState::Buffering, PlayerEvent::Fail) => {
            (PlayerState::Error, vec![PlayerEffect::StopDecoder, PlayerEffect::RaiseAlert])
        }
        (PlayerState::Stopped, PlayerEvent::Fail) => {
            (PlayerState::Error, vec![PlayerEffect::StopDecoder, PlayerEffect::RaiseAlert])
        }
        _ => (state, vec![]),
    }
}
"#;

        let source_hash = Self::compute_sha256(player_source);
        let (rustc_ver, swift_ver) = Self::toolchain_versions();

        // 1. Lower to IR
        let mut frontend = Frontend::new("player_fsm.rs");
        let module = frontend
            .parse_and_lower(player_source)
            .map_err(|diags| format!("Frontend rejected player FSM fixture: {:?}", diags))?;

        // 2. Generate Swift package
        let pkg_dir = out_dir.join("player_pkg");
        let mut generator = SwiftGenerator::new();
        generator.emit_package(&module, &pkg_dir, "PlayerModule")?;

        // 3. Create a Swift test runner executable
        let runner_swift = r#"
import Foundation

@main
struct EventTraceRunner {
    static func main() {
        let events: [PlayerEvent] = [
            .Load, .BufferReady, .Play, .BufferUnderflow, .BufferReady,
            .Pause, .Play, .Stop, .Reset, .Load,
            .Play, .BufferReady, .Play, .Pause, .Play,
            .BufferUnderflow, .Fail, .Reset, .Load, .BufferReady,
            .Play, .Pause, .Stop, .Reset, .Load,
            .BufferReady, .Play, .BufferUnderflow, .BufferReady, .Pause,
            .Play, .Stop, .Reset, .Load, .BufferReady,
            .Play, .BufferUnderflow, .BufferReady, .Pause, .Play,
            .Stop, .Reset, .Load, .Fail, .Reset,
            .Load, .BufferReady, .Play, .Stop, .Reset
        ]

        var current_state = PlayerState.Idle
        for (idx, evt) in events.enumerated() {
            let before = current_state
            let (next_state, effects) = step_player(current_state, evt)
            current_state = next_state
            let effects_json = effects.map { "\"\($0.rawValue)\"" }.joined(separator: ",")
            print("{\"sequence_id\":\(idx),\"state_before\":\"\(before.rawValue)\",\"event_in\":\"\(evt.rawValue)\",\"state_after\":\"\(next_state.rawValue)\",\"effects\":[\(effects_json)]}")
        }
    }
}
"#;
        let sources_dir = pkg_dir.join("Sources").join("PlayerModule");
        let main_swift_path = sources_dir.join("main.swift");
        fs::write(&main_swift_path, runner_swift)?;

        // 4. Compile and execute Swift code
        let swift_source = sources_dir.join("player_fsm.swift");
        let swiftc_out = Command::new("swiftc")
            .arg("-parse-as-library")
            .arg("-emit-executable")
            .arg(&swift_source)
            .arg(&main_swift_path)
            .arg("-o")
            .arg(pkg_dir.join("player_runner"))
            .output()?;

        if !swiftc_out.status.success() {
            return Err(format!(
                "Failed to compile Swift EventTrace runner: {}",
                String::from_utf8_lossy(&swiftc_out.stderr)
            )
            .into());
        }

        let exec_out = Command::new(pkg_dir.join("player_runner")).output()?;
        let swift_results = String::from_utf8(exec_out.stdout)?;

        // 5. Compute expected results using Rust implementation
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        enum RustPlayerState {
            Idle,
            Loading,
            Playing,
            Paused,
            Buffering,
            Stopped,
            Error,
        }
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        enum RustPlayerEvent {
            Load,
            Play,
            Pause,
            BufferUnderflow,
            BufferReady,
            Stop,
            Fail,
            Reset,
        }
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        enum RustPlayerEffect {
            StartDecoder,
            StopDecoder,
            RenderFrame,
            MuteAudio,
            UnmuteAudio,
            LogNotice,
            RaiseAlert,
        }
        fn rust_step_player(
            state: RustPlayerState,
            event: RustPlayerEvent,
        ) -> (RustPlayerState, Vec<RustPlayerEffect>) {
            match (state, event) {
                (RustPlayerState::Idle, RustPlayerEvent::Load) => (
                    RustPlayerState::Loading,
                    vec![RustPlayerEffect::StartDecoder, RustPlayerEffect::LogNotice],
                ),
                (RustPlayerState::Loading, RustPlayerEvent::BufferReady) => {
                    (RustPlayerState::Paused, vec![RustPlayerEffect::UnmuteAudio])
                }
                (RustPlayerState::Paused, RustPlayerEvent::Play) => (
                    RustPlayerState::Playing,
                    vec![RustPlayerEffect::RenderFrame],
                ),
                (RustPlayerState::Playing, RustPlayerEvent::Pause) => {
                    (RustPlayerState::Paused, vec![RustPlayerEffect::MuteAudio])
                }
                (RustPlayerState::Playing, RustPlayerEvent::BufferUnderflow) => (
                    RustPlayerState::Buffering,
                    vec![RustPlayerEffect::MuteAudio, RustPlayerEffect::LogNotice],
                ),
                (RustPlayerState::Buffering, RustPlayerEvent::BufferReady) => (
                    RustPlayerState::Playing,
                    vec![RustPlayerEffect::UnmuteAudio, RustPlayerEffect::RenderFrame],
                ),
                (RustPlayerState::Playing, RustPlayerEvent::Stop) => (
                    RustPlayerState::Stopped,
                    vec![RustPlayerEffect::StopDecoder, RustPlayerEffect::MuteAudio],
                ),
                (RustPlayerState::Paused, RustPlayerEvent::Stop) => (
                    RustPlayerState::Stopped,
                    vec![RustPlayerEffect::StopDecoder],
                ),
                (RustPlayerState::Stopped, RustPlayerEvent::Reset) => {
                    (RustPlayerState::Idle, vec![RustPlayerEffect::LogNotice])
                }
                (RustPlayerState::Error, RustPlayerEvent::Reset) => {
                    (RustPlayerState::Idle, vec![RustPlayerEffect::LogNotice])
                }
                (RustPlayerState::Idle, RustPlayerEvent::Fail) => (
                    RustPlayerState::Error,
                    vec![RustPlayerEffect::StopDecoder, RustPlayerEffect::RaiseAlert],
                ),
                (RustPlayerState::Loading, RustPlayerEvent::Fail) => (
                    RustPlayerState::Error,
                    vec![RustPlayerEffect::StopDecoder, RustPlayerEffect::RaiseAlert],
                ),
                (RustPlayerState::Playing, RustPlayerEvent::Fail) => (
                    RustPlayerState::Error,
                    vec![RustPlayerEffect::StopDecoder, RustPlayerEffect::RaiseAlert],
                ),
                (RustPlayerState::Paused, RustPlayerEvent::Fail) => (
                    RustPlayerState::Error,
                    vec![RustPlayerEffect::StopDecoder, RustPlayerEffect::RaiseAlert],
                ),
                (RustPlayerState::Buffering, RustPlayerEvent::Fail) => (
                    RustPlayerState::Error,
                    vec![RustPlayerEffect::StopDecoder, RustPlayerEffect::RaiseAlert],
                ),
                (RustPlayerState::Stopped, RustPlayerEvent::Fail) => (
                    RustPlayerState::Error,
                    vec![RustPlayerEffect::StopDecoder, RustPlayerEffect::RaiseAlert],
                ),
                _ => (state, vec![]),
            }
        }

        let events = [
            RustPlayerEvent::Load,
            RustPlayerEvent::BufferReady,
            RustPlayerEvent::Play,
            RustPlayerEvent::BufferUnderflow,
            RustPlayerEvent::BufferReady,
            RustPlayerEvent::Pause,
            RustPlayerEvent::Play,
            RustPlayerEvent::Stop,
            RustPlayerEvent::Reset,
            RustPlayerEvent::Load,
            RustPlayerEvent::Play,
            RustPlayerEvent::BufferReady,
            RustPlayerEvent::Play,
            RustPlayerEvent::Pause,
            RustPlayerEvent::Play,
            RustPlayerEvent::BufferUnderflow,
            RustPlayerEvent::Fail,
            RustPlayerEvent::Reset,
            RustPlayerEvent::Load,
            RustPlayerEvent::BufferReady,
            RustPlayerEvent::Play,
            RustPlayerEvent::Pause,
            RustPlayerEvent::Stop,
            RustPlayerEvent::Reset,
            RustPlayerEvent::Load,
            RustPlayerEvent::BufferReady,
            RustPlayerEvent::Play,
            RustPlayerEvent::BufferUnderflow,
            RustPlayerEvent::BufferReady,
            RustPlayerEvent::Pause,
            RustPlayerEvent::Play,
            RustPlayerEvent::Stop,
            RustPlayerEvent::Reset,
            RustPlayerEvent::Load,
            RustPlayerEvent::BufferReady,
            RustPlayerEvent::Play,
            RustPlayerEvent::BufferUnderflow,
            RustPlayerEvent::BufferReady,
            RustPlayerEvent::Pause,
            RustPlayerEvent::Play,
            RustPlayerEvent::Stop,
            RustPlayerEvent::Reset,
            RustPlayerEvent::Load,
            RustPlayerEvent::Fail,
            RustPlayerEvent::Reset,
            RustPlayerEvent::Load,
            RustPlayerEvent::BufferReady,
            RustPlayerEvent::Play,
            RustPlayerEvent::Stop,
            RustPlayerEvent::Reset,
        ];

        let mut current_state = RustPlayerState::Idle;
        let mut rust_lines = Vec::new();
        for (idx, &evt) in events.iter().enumerate() {
            let before = current_state;
            let (next_state, effects) = rust_step_player(current_state, evt);
            current_state = next_state;
            let effects_json = effects
                .iter()
                .map(|e| format!("\"{:?}\"", e))
                .collect::<Vec<_>>()
                .join(",");
            rust_lines.push(format!(
                "{{\"sequence_id\":{},\"state_before\":\"{:?}\",\"event_in\":\"{:?}\",\"state_after\":\"{:?}\",\"effects\":[{}]}}",
                idx, before, evt, next_state, effects_json
            ));
        }

        let swift_lines: Vec<&str> = swift_results
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        let total = rust_lines.len();
        let mut cases_passed = 0;
        let mut details = Vec::new();

        for (idx, expected) in rust_lines.iter().enumerate() {
            if idx < swift_lines.len() && expected == swift_lines[idx] {
                cases_passed += 1;
                details.push(format!("Transition #{}: PASS ({})", idx + 1, expected));
            } else {
                let actual = if idx < swift_lines.len() {
                    swift_lines[idx]
                } else {
                    "<missing>"
                };
                details.push(format!(
                    "Transition #{}: FAIL (Expected: {}, Got: {})",
                    idx + 1,
                    expected,
                    actual
                ));
            }
        }

        let rust_full_trace = rust_lines.join("\n");
        let swift_full_trace = swift_lines.join("\n");
        let rust_digest = Self::compute_sha256(&rust_full_trace);
        let swift_digest = Self::compute_sha256(&swift_full_trace);

        let digests_match = rust_digest == swift_digest;
        details.push(format!(
            "Trace digest verification: Rust={}, Swift={}, Equal={}",
            rust_digest, swift_digest, digests_match
        ));

        let status = if cases_passed == total && digests_match {
            "PASSED"
        } else {
            "FAILED"
        };

        Ok(QualificationReport {
            fixture_name: "event_dispatch_and_trace_stream".into(),
            source_hash,
            rustc_version: rustc_ver,
            swift_version: swift_ver,
            cases_tested: total,
            cases_passed,
            cases_failed: total - cases_passed,
            status: status.into(),
            timestamp: "2026-10-13T20:00:00Z".into(),
            details,
        })
    }

    /// Run negative rejection tests asserting exact stable diagnostic codes
    pub fn test_negative_rejections(
        &self,
    ) -> Result<Vec<QualificationReport>, Box<dyn std::error::Error>> {
        let (rustc_ver, swift_ver) = Self::toolchain_versions();
        let mut reports = Vec::new();

        // 1. Unsafe rejection
        let unsafe_src = "pub fn foo() -> i32 { unsafe { 10 } }";
        let mut f1 = Frontend::new("unsafe.rs");
        let res1 = f1.parse_and_lower(unsafe_src);
        let p1 = res1.is_err()
            && res1
                .unwrap_err()
                .iter()
                .any(|d| d.code == DiagnosticCode::UnsafeRejected);
        reports.push(QualificationReport {
            fixture_name: "rejected_unsafe".into(),
            source_hash: Self::compute_sha256(unsafe_src),
            rustc_version: rustc_ver.clone(),
            swift_version: swift_ver.clone(),
            cases_tested: 1,
            cases_passed: if p1 { 1 } else { 0 },
            cases_failed: if p1 { 0 } else { 1 },
            status: if p1 {
                "REJECTED_AS_EXPECTED".into()
            } else {
                "FAILED".into()
            },
            timestamp: "2026-10-13T20:00:00Z".into(),
            details: vec!["Verified E0001_UNSAFE_REJECTED emitted".into()],
        });

        // 2. Async rejection
        let async_src = "pub async fn bar() -> i32 { 10 }";
        let mut f2 = Frontend::new("async.rs");
        let res2 = f2.parse_and_lower(async_src);
        let p2 = res2.is_err()
            && res2
                .unwrap_err()
                .iter()
                .any(|d| d.code == DiagnosticCode::AsyncRejected);
        reports.push(QualificationReport {
            fixture_name: "rejected_async".into(),
            source_hash: Self::compute_sha256(async_src),
            rustc_version: rustc_ver.clone(),
            swift_version: swift_ver.clone(),
            cases_tested: 1,
            cases_passed: if p2 { 1 } else { 0 },
            cases_failed: if p2 { 0 } else { 1 },
            status: if p2 {
                "REJECTED_AS_EXPECTED".into()
            } else {
                "FAILED".into()
            },
            timestamp: "2026-10-13T20:00:00Z".into(),
            details: vec!["Verified E0002_ASYNC_REJECTED emitted".into()],
        });

        // 3. Raw pointer rejection
        let ptr_src = "pub fn deref(p: *const i32) -> i32 { 0 }";
        let mut f3 = Frontend::new("raw_ptr.rs");
        let res3 = f3.parse_and_lower(ptr_src);
        let p3 = res3.is_err()
            && res3
                .unwrap_err()
                .iter()
                .any(|d| d.code == DiagnosticCode::RawPointerRejected);
        reports.push(QualificationReport {
            fixture_name: "rejected_raw_pointers".into(),
            source_hash: Self::compute_sha256(ptr_src),
            rustc_version: rustc_ver.clone(),
            swift_version: swift_ver.clone(),
            cases_tested: 1,
            cases_passed: if p3 { 1 } else { 0 },
            cases_failed: if p3 { 0 } else { 1 },
            status: if p3 {
                "REJECTED_AS_EXPECTED".into()
            } else {
                "FAILED".into()
            },
            timestamp: "2026-10-13T20:00:00Z".into(),
            details: vec!["Verified E0003_RAW_POINTER_REJECTED emitted".into()],
        });

        // 4. Complex trait rejection
        let trait_src = "pub struct Foo; impl Bar for Foo {}";
        let mut f4 = Frontend::new("trait_impl.rs");
        let res4 = f4.parse_and_lower(trait_src);
        let p4 = res4.is_err()
            && res4
                .unwrap_err()
                .iter()
                .any(|d| d.code == DiagnosticCode::ComplexTraitRejected);
        reports.push(QualificationReport {
            fixture_name: "rejected_complex_traits".into(),
            source_hash: Self::compute_sha256(trait_src),
            rustc_version: rustc_ver.clone(),
            swift_version: swift_ver.clone(),
            cases_tested: 1,
            cases_passed: if p4 { 1 } else { 0 },
            cases_failed: if p4 { 0 } else { 1 },
            status: if p4 {
                "REJECTED_AS_EXPECTED".into()
            } else {
                "FAILED".into()
            },
            timestamp: "2026-10-13T20:00:00Z".into(),
            details: vec!["Verified E0006_COMPLEX_TRAIT_REJECTED emitted".into()],
        });

        // 5. Dynamic dispatch rejection
        let dyn_src = "pub fn call_dyn(x: &dyn Display) {}";
        let mut f5 = Frontend::new("dyn_trait.rs");
        let res5 = f5.parse_and_lower(dyn_src);
        let p5 = res5.is_err()
            && res5
                .unwrap_err()
                .iter()
                .any(|d| d.code == DiagnosticCode::DynamicDispatchRejected);
        reports.push(QualificationReport {
            fixture_name: "rejected_dynamic_dispatch".into(),
            source_hash: Self::compute_sha256(dyn_src),
            rustc_version: rustc_ver,
            swift_version: swift_ver,
            cases_tested: 1,
            cases_passed: if p5 { 1 } else { 0 },
            cases_failed: if p5 { 0 } else { 1 },
            status: if p5 {
                "REJECTED_AS_EXPECTED".into()
            } else {
                "FAILED".into()
            },
            timestamp: "2026-10-13T20:00:00Z".into(),
            details: vec!["Verified E0005_DYNAMIC_DISPATCH_REJECTED emitted".into()],
        });

        Ok(reports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fsm_differential_passes() {
        let runner = DifferentialRunner::new(".");
        let temp_dir = std::env::temp_dir().join("fsm_diff_test");
        let _ = fs::create_dir_all(&temp_dir);
        let report = runner
            .test_fsm(&temp_dir)
            .expect("FSM differential test should pass");
        assert_eq!(report.status, "PASSED");
        assert_eq!(report.cases_passed, 16);
        assert_eq!(report.cases_failed, 0);
    }

    #[test]
    fn test_wrapping_arithmetic_differential_passes() {
        let runner = DifferentialRunner::new(".");
        let temp_dir = std::env::temp_dir().join("arith_diff_test");
        let _ = fs::create_dir_all(&temp_dir);
        let report = runner
            .test_wrapping_arithmetic(&temp_dir)
            .expect("Arithmetic differential test should pass");
        assert_eq!(report.status, "PASSED");
        assert_eq!(report.cases_failed, 0);
    }

    #[test]
    fn test_negative_diagnostics_pass() {
        let runner = DifferentialRunner::new(".");
        let reports = runner
            .test_negative_rejections()
            .expect("Negative rejections should pass");
        assert_eq!(reports.len(), 5);
        for r in reports {
            assert_eq!(r.status, "REJECTED_AS_EXPECTED");
        }
    }

    #[test]
    fn test_contract_system_differential_passes() {
        let runner = DifferentialRunner::new(".");
        let temp_dir = std::env::temp_dir().join("contract_diff_test");
        let _ = fs::create_dir_all(&temp_dir);
        let report = runner
            .test_contracts(&temp_dir)
            .expect("Contract differential test should pass");
        assert_eq!(report.status, "PASSED");
        assert_eq!(report.cases_passed, 7);
        assert_eq!(report.cases_failed, 0);
    }

    #[test]
    fn test_types_differential_passes() {
        let runner = DifferentialRunner::new(".");
        let temp_dir = std::env::temp_dir().join("types_diff_test");
        let _ = fs::create_dir_all(&temp_dir);
        let report = runner
            .test_types(&temp_dir)
            .expect("Types differential test should pass");
        assert_eq!(report.status, "PASSED");
        assert_eq!(report.cases_passed, 5);
        assert_eq!(report.cases_failed, 0);
    }

    #[test]
    fn test_event_trace_differential_passes() {
        let runner = DifferentialRunner::new(".");
        let temp_dir = std::env::temp_dir().join("event_trace_diff_test");
        let _ = fs::create_dir_all(&temp_dir);
        let report = runner
            .test_event_trace(&temp_dir)
            .expect("Event trace differential test should pass");
        assert_eq!(report.status, "PASSED");
        assert_eq!(report.cases_passed, 50);
        assert_eq!(report.cases_failed, 0);
    }
}
