//! Model Context Protocol (MCP) server library for the rust-swift compiler.
//!
//! Exposes tool endpoints (`rust_swift_check`, `rust_swift_compile`, `rust_swift_proof`, `rust_swift_explain`)
//! over standard JSON-RPC 2.0 transport for AI agents and developer tools (Claude, Codex, Antigravity, Cursor).

use rust_swift_codegen::SwiftGenerator;
use rust_swift_frontend::Frontend;
use rust_swift_qualification::proof::ProofVerifier;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

/// Server information adhering to the Model Context Protocol specification.
pub const SERVER_NAME: &str = "rust-swift-mcp";
pub const SERVER_VERSION: &str = "0.1.0";
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Diagnostic report formatted for LLM consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDiagnostic {
    pub code: String,
    pub message: String,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_remedy: Option<String>,
}

/// JSON response schema for pre-flight boundary check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryCheckResult {
    pub status: String,
    pub file: String,
    pub items_admitted: usize,
    pub diagnostics: Vec<AgentDiagnostic>,
    pub summary: String,
}

/// JSON response schema for compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swift_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_files: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
    pub diagnostics: Vec<AgentDiagnostic>,
}

/// Main MCP message router and dispatcher.
pub struct McpServer;

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    /// Create a new McpServer instance.
    pub fn new() -> Self {
        Self
    }

    /// Run the stdio JSON-RPC loop.
    pub fn run_stdio() -> Result<(), Box<dyn std::error::Error>> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let reader = stdin.lock();

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(response) = Self::handle_message(trimmed) {
                stdout.write_all(response.as_bytes())?;
                stdout.write_all(b"\n")?;
                stdout.flush()?;
            }
        }

        Ok(())
    }

    /// Process a single incoming JSON-RPC string message.
    pub fn handle_message(raw_msg: &str) -> Option<String> {
        let parsed: Value = match serde_json::from_str(raw_msg) {
            Ok(v) => v,
            Err(e) => {
                return Some(
                    json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": {
                            "code": -32700,
                            "message": format!("Parse error: {}", e)
                        }
                    })
                    .to_string(),
                );
            }
        };

        let method = parsed.get("method").and_then(|m| m.as_str());
        let id = parsed.get("id").cloned();

        match method {
            Some("initialize") => {
                let res = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": PROTOCOL_VERSION,
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": SERVER_NAME,
                            "version": SERVER_VERSION
                        }
                    }
                });
                Some(res.to_string())
            }
            Some("notifications/initialized") | Some("initialized") => {
                // Notifications do not return a response
                None
            }
            Some("ping") => {
                let res = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {}
                });
                Some(res.to_string())
            }
            Some("tools/list") => {
                let tools = Self::list_tools();
                let res = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": tools
                    }
                });
                Some(res.to_string())
            }
            Some("tools/call") => {
                let params = parsed.get("params");
                let tool_name = params
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let arguments = params
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or(json!({}));

                let call_result = Self::execute_tool(tool_name, &arguments);
                let res = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": call_result
                });
                Some(res.to_string())
            }
            Some(other) => Some(
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Method not found: {}", other)
                    }
                })
                .to_string(),
            ),
            None => None,
        }
    }

    /// List tool schemas compliant with MCP.
    fn list_tools() -> Value {
        json!([
            {
                "name": "rust_swift_check",
                "description": "Inspect whether a Rust source snippet or file satisfies the admitted rust-swift Language Matrix subset. Returns structured JSON diagnostics with stable error codes (E0001..E0015) and actionable remedy recommendations for auto-healing.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "string",
                            "description": "Rust source code snippet to check (optional if file_path is provided)"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Path to a Rust source file on disk to check (optional if source is provided)"
                        },
                        "codable": {
                            "type": "boolean",
                            "description": "Whether to force Codable synthesis"
                        },
                        "observable": {
                            "type": "boolean",
                            "description": "Whether to force SwiftUI @Observable macro synthesis"
                        }
                    }
                }
            },
            {
                "name": "rust_swift_compile",
                "description": "Compiles an admitted Rust source snippet or file into 100% native, zero-FFI Swift code or a complete Swift Package Manager (SPM) package with Package.swift and tests.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "string",
                            "description": "Rust source code to compile (optional if file_path is provided)"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Path to a Rust source file on disk (optional if source is provided)"
                        },
                        "package_name": {
                            "type": "string",
                            "description": "Name for the generated Swift package/module (defaults to 'GeneratedLogic')"
                        },
                        "output_dir": {
                            "type": "string",
                            "description": "Output directory path on disk. If provided, writes the package to disk. If omitted, returns generated Swift code directly in the response."
                        },
                        "spm": {
                            "type": "boolean",
                            "description": "Whether to generate full SPM layout (Package.swift, Sources, Tests). Default is true if output_dir is given."
                        },
                        "codable": {
                            "type": "boolean",
                            "description": "Force Swift Codable conformance across all types"
                        },
                        "observable": {
                            "type": "boolean",
                            "description": "Synthesize SwiftUI @Observable classes for state models"
                        },
                        "source_locations": {
                            "type": "boolean",
                            "description": "Emit #sourceLocation directives for Xcode/LLDB debugging"
                        }
                    }
                }
            },
            {
                "name": "rust_swift_proof",
                "description": "Formally certifies safety, integer wrapping preservation, FSM totality, and contract invariants for a Rust module, generating an epistemic proof dossier receipt.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "string",
                            "description": "Rust source code to certify"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Path to Rust source file on disk"
                        }
                    }
                }
            },
            {
                "name": "rust_swift_explain",
                "description": "Explains how a specific Rust construct or rejection diagnostic code (E0001..E0015) is handled by the rust-swift compiler, including auto-healing recommendations for LLMs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Diagnostic code (e.g. 'E0001', 'E0006') or feature keyword (e.g. 'bitflags', 'observable', 'contracts', 'traits', 'wrapping')"
                        }
                    },
                    "required": ["query"]
                }
            }
        ])
    }

    /// Dispatch tool calls and return MCP formatted content.
    fn execute_tool(name: &str, args: &Value) -> Value {
        match name {
            "rust_swift_check" => Self::tool_check(args),
            "rust_swift_compile" => Self::tool_compile(args),
            "rust_swift_proof" => Self::tool_proof(args),
            "rust_swift_explain" => Self::tool_explain(args),
            other => json!({
                "isError": true,
                "content": [
                    {
                        "type": "text",
                        "text": format!("Unknown tool: {}", other)
                    }
                ]
            }),
        }
    }

    fn extract_source(args: &Value) -> Result<(String, String), String> {
        if let Some(src) = args.get("source").and_then(|s| s.as_str()) {
            Ok((src.to_string(), "snippet.rs".to_string()))
        } else if let Some(path_str) = args.get("file_path").and_then(|p| p.as_str()) {
            let path = Path::new(path_str);
            if !path.exists() {
                return Err(format!("File does not exist: {}", path_str));
            }
            let content =
                fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
            Ok((content, path_str.to_string()))
        } else {
            Err("Either 'source' code string or 'file_path' must be provided.".to_string())
        }
    }

    fn tool_check(args: &Value) -> Value {
        let (source, file_name) = match Self::extract_source(args) {
            Ok(pair) => pair,
            Err(e) => {
                return json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": e }]
                });
            }
        };

        let codable = args
            .get("codable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let observable = args
            .get("observable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut frontend = Frontend::new(&file_name)
            .with_default_codable(codable)
            .with_default_observable(observable);

        match frontend.parse_and_lower(&source) {
            Ok(module) => {
                let res = BoundaryCheckResult {
                    status: "admitted".to_string(),
                    file: file_name,
                    items_admitted: module.items.len(),
                    diagnostics: vec![],
                    summary: format!(
                        "Source is valid under the rust-swift Language Matrix. Admitted {} top-level item(s).",
                        module.items.len()
                    ),
                };
                json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&res).unwrap()
                    }]
                })
            }
            Err(diags) => {
                let agent_diags: Vec<AgentDiagnostic> = diags
                    .into_iter()
                    .map(|d| AgentDiagnostic {
                        code: d.code.as_str().to_string(),
                        message: d.message,
                        file: d.span.file,
                        start_line: d.span.start_line,
                        end_line: d.span.end_line,
                        suggested_remedy: d.suggested_alternative,
                    })
                    .collect();

                let res = BoundaryCheckResult {
                    status: "rejected".to_string(),
                    file: file_name,
                    items_admitted: 0,
                    summary: format!(
                        "Rejected with {} diagnostic(s) outside verified Language Matrix subset.",
                        agent_diags.len()
                    ),
                    diagnostics: agent_diags,
                };
                json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&res).unwrap()
                    }]
                })
            }
        }
    }

    fn tool_compile(args: &Value) -> Value {
        let (source, file_name) = match Self::extract_source(args) {
            Ok(pair) => pair,
            Err(e) => {
                return json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": e }]
                });
            }
        };

        let package_name = args
            .get("package_name")
            .and_then(|v| v.as_str())
            .unwrap_or("GeneratedLogic");
        let output_dir = args.get("output_dir").and_then(|v| v.as_str());
        let spm = args
            .get("spm")
            .and_then(|v| v.as_bool())
            .unwrap_or(output_dir.is_some());
        let codable = args
            .get("codable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let observable = args
            .get("observable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let source_locations = args
            .get("source_locations")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        let source_hash = format!("{:x}", hasher.finalize());

        let mut frontend = Frontend::new(&file_name)
            .with_default_codable(codable)
            .with_default_observable(observable);

        let module = match frontend.parse_and_lower(&source) {
            Ok(m) => m,
            Err(diags) => {
                let agent_diags: Vec<AgentDiagnostic> = diags
                    .into_iter()
                    .map(|d| AgentDiagnostic {
                        code: d.code.as_str().to_string(),
                        message: d.message,
                        file: d.span.file,
                        start_line: d.span.start_line,
                        end_line: d.span.end_line,
                        suggested_remedy: d.suggested_alternative,
                    })
                    .collect();

                let res = CompilationResult {
                    status: "rejected".to_string(),
                    swift_source: None,
                    package_name: None,
                    output_dir: None,
                    generated_files: None,
                    source_hash: Some(source_hash),
                    diagnostics: agent_diags,
                };
                return json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&res).unwrap()
                    }]
                });
            }
        };

        let mut generator = SwiftGenerator::new()
            .with_source_locations(source_locations)
            .with_observable(observable);

        if let Some(out_path_str) = output_dir {
            let out_path = Path::new(out_path_str);
            if spm {
                match generator.emit_package(&module, out_path, package_name) {
                    Ok(manifest) => {
                        let res = CompilationResult {
                            status: "success".to_string(),
                            swift_source: None,
                            package_name: Some(package_name.to_string()),
                            output_dir: Some(out_path_str.to_string()),
                            generated_files: Some(manifest.generated_files),
                            source_hash: Some(source_hash),
                            diagnostics: vec![],
                        };
                        json!({
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&res).unwrap()
                            }]
                        })
                    }
                    Err(e) => json!({
                        "isError": true,
                        "content": [{ "type": "text", "text": format!("Package generation failed: {}", e) }]
                    }),
                }
            } else {
                let (code, _symbols) = generator.generate_module(&module);
                let _ = fs::create_dir_all(out_path);
                let file_path = out_path.join(format!("{}.swift", module.name));
                let _ = fs::write(&file_path, &code);

                let res = CompilationResult {
                    status: "success".to_string(),
                    swift_source: Some(code),
                    package_name: Some(package_name.to_string()),
                    output_dir: Some(out_path_str.to_string()),
                    generated_files: Some(vec![file_path.to_string_lossy().to_string()]),
                    source_hash: Some(source_hash),
                    diagnostics: vec![],
                };
                json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&res).unwrap()
                    }]
                })
            }
        } else {
            let (code, _symbols) = generator.generate_module(&module);
            let res = CompilationResult {
                status: "success".to_string(),
                swift_source: Some(code),
                package_name: Some(package_name.to_string()),
                output_dir: None,
                generated_files: None,
                source_hash: Some(source_hash),
                diagnostics: vec![],
            };
            json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&res).unwrap()
                }]
            })
        }
    }

    fn tool_proof(args: &Value) -> Value {
        let (source, file_name) = match Self::extract_source(args) {
            Ok(pair) => pair,
            Err(e) => {
                return json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": e }]
                });
            }
        };

        let mut frontend = Frontend::new(&file_name);
        let module = match frontend.parse_and_lower(&source) {
            Ok(m) => m,
            Err(diags) => {
                return json!({
                    "isError": true,
                    "content": [{
                        "type": "text",
                        "text": format!("Proof generation failed: module rejected with {} diagnostics", diags.len())
                    }]
                });
            }
        };

        let receipt = ProofVerifier::verify(&module, &source);
        let receipt_json = serde_json::to_string_pretty(&receipt).unwrap();
        json!({
            "content": [{
                "type": "text",
                "text": receipt_json
            }]
        })
    }

    fn tool_explain(args: &Value) -> Value {
        let query = args
            .get("query")
            .and_then(|q| q.as_str())
            .unwrap_or("")
            .trim();
        let upper = query.to_uppercase();

        let explanation = match upper.as_str() {
            "E0001" | "E0001_UNSAFE_REJECTED" | "UNSAFE" => {
                "## E0001_UNSAFE_REJECTED\n\n\
                 **Rule**: `unsafe` blocks, `unsafe fn`, and raw pointer dereferences are strictly prohibited.\n\n\
                 **Reasoning**: rust-swift guarantees zero undefined behavior and emits 100% safe Swift value types.\n\n\
                 **Agent Remedy**:\n\
                 - Replace raw pointer arithmetic with safe array indexing or Vec operations.\n\
                 - Replace `unsafe` transmutation with typed enum variants or serde serialization.\n\
                 - Use safe wrapping arithmetic (`wrapping_add`, `wrapping_sub`, `wrapping_mul`)."
            }
            "E0002" | "E0002_ASYNC_REJECTED" | "ASYNC" => {
                "## E0002_ASYNC_REJECTED\n\n\
                 **Rule**: Native `async fn` is rejected in v0.1.\n\n\
                 **Reasoning**: Swift 6 strict concurrency (Actor isolation, Sendable checks) requires domain models to stay synchronous.\n\n\
                 **Agent Remedy**:\n\
                 - Write synchronous domain state engines and algorithms in Rust.\n\
                 - Handle network I/O, Task dispatch, and async awaits in the outer Swift application layer."
            }
            "E0003" | "E0003_RAW_POINTER_REJECTED" => {
                "## E0003_RAW_POINTER_REJECTED\n\n\
                 **Rule**: Raw pointers (`*const T`, `*mut T`) are forbidden.\n\n\
                 **Agent Remedy**: Use safe Rust references (`&T`, `&mut T`), `Vec<T>`, or `Option<T>`."
            }
            "E0006" | "E0006_COMPLEX_TRAIT_REJECTED" => {
                "## E0006_COMPLEX_TRAIT_REJECTED\n\n\
                 **Rule**: Generic trait parameters (`trait Foo<T>`) are rejected. Associated types (`type Item;`) are fully supported!\n\n\
                 **Agent Remedy**:\n\
                 - Refactor generic trait parameters into associated types:\n\
                   ```rust\n\
                   // Instead of: pub trait Container<T> { ... }\n\
                   // Use:\n\
                   pub trait Container {\n\
                       type Item;\n\
                       fn get(&self) -> Self::Item;\n\
                   }\n\
                   ```"
            }
            "E0008" | "E0008_UNSUPPORTED_TYPE_REJECTED" => {
                "## E0008_UNSUPPORTED_TYPE_REJECTED\n\n\
                 **Rule**: Tuple structs (`struct Foo(i32, String);`) are rejected.\n\n\
                 **Agent Remedy**: Use structs with named fields (`struct Foo { pub a: i32, pub b: String }`)."
            }
            "BITFLAGS" | "OPTIONSET" => {
                "## Bitflags & Mask Types (Swift OptionSet)\n\n\
                 Annotate integer mask structs with `#[bitflags]`:\n\
                 ```rust\n\
                 #[bitflags]\n\
                 pub struct Permissions {\n\
                     pub raw_value: u32,\n\
                 }\n\
                 impl Permissions {\n\
                     pub const READ: Permissions = Permissions { raw_value: 1 };\n\
                     pub const WRITE: Permissions = Permissions { raw_value: 2 };\n\
                 }\n\
                 ```\n\
                 Lowers to native Swift:\n\
                 ```swift\n\
                 public struct Permissions: OptionSet, Sendable {\n\
                     public let rawValue: UInt32\n\
                     public init(rawValue: UInt32) { self.rawValue = rawValue }\n\
                     public static let READ = Permissions(rawValue: 1)\n\
                     public static let WRITE = Permissions(rawValue: 2)\n\
                 }\n\
                 ```\n\
                 Synthesizes `|` and `&` operators, `.contains(...)`, and array literal initialization `[.READ, .WRITE]`."
            }
            "OBSERVABLE" | "SWIFTUI" => {
                "## SwiftUI @Observable Architecture\n\n\
                 Annotate struct state models with `#[observable]`:\n\
                 ```rust\n\
                 #[observable]\n\
                 pub struct UserState {\n\
                     pub id: i64,\n\
                     pub name: String,\n\
                 }\n\
                 ```\n\
                 Lowers directly to Swift 5.9+ Observation framework:\n\
                 ```swift\n\
                 @Observable\n\
                 public final class UserState: Identifiable {\n\
                     public var id: Int64\n\
                     public var name: String\n\
                 }\n\
                 ```"
            }
            "WRAPPING" | "OVERFLOW" | "ARITHMETIC" => {
                "## Integer Wrapping Preservation\n\n\
                 Rust wrapping methods map directly to Swift native overflow operators:\n\
                 - `a.wrapping_add(b)` -> `(a &+ b)`\n\
                 - `a.wrapping_sub(b)` -> `(a &- b)`\n\
                 - `a.wrapping_mul(b)` -> `(a &* b)`\n\
                 Regular arithmetic (`+`, `-`, `*`) generates checked operations that trap upon overflow in both languages."
            }
            _ => {
                "## rust-swift Admission Boundary Summary\n\n\
                 **Supported Features**:\n\
                 - Primitive numbers (`i8`..`i64`, `u8`..`u64`, `usize`, `isize`, `f32`, `f64`, `bool`)\n\
                 - Structs with named fields (`#[derive(Codable)]`, `#[observable]`, `#[bitflags]`)\n\
                 - Enums (fieldless and payload variants)\n\
                 - Traits with default methods and associated types (`type Item;`)\n\
                 - Collections: `Vec<T>`, slices, arrays, `Option<T>`, `Result<T, E>`, `String`\n\
                 - Inherent impl blocks and associated constants\n\
                 - Region borrowing exclusivity (`&mut param` -> `inout param`)\n\
                 - Contract assertions (`assert!`, `debug_assert!` -> `precondition`, `assert`)\n\n\
                 **Rejection Codes**:\n\
                 - `E0001`: Unsafe blocks\n\
                 - `E0002`: Async functions\n\
                 - `E0003`: Raw pointers\n\
                 - `E0004`: Unadmitted macros\n\
                 - `E0005`: Dynamic dispatch (`dyn Trait`)\n\
                 - `E0006`: Generic trait parameters\n\
                 - `E0008`: Tuple structs\n\
                 - `E0011`: Unspecialized type parameters on functions"
            }
        };

        json!({
            "content": [{
                "type": "text",
                "text": explanation
            }]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_initialize() {
        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            }
        })
        .to_string();

        let resp = McpServer::handle_message(&init_req).expect("Expected response for initialize");
        let val: Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(val["jsonrpc"], "2.0");
        assert_eq!(val["id"], 1);
        assert_eq!(val["result"]["serverInfo"]["name"], "rust-swift-mcp");
    }

    #[test]
    fn test_mcp_tools_list() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        })
        .to_string();

        let resp = McpServer::handle_message(&req).expect("Expected tools/list response");
        let val: Value = serde_json::from_str(&resp).unwrap();
        let tools = val["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|t| t["name"] == "rust_swift_check"));
        assert!(tools.iter().any(|t| t["name"] == "rust_swift_compile"));
        assert!(tools.iter().any(|t| t["name"] == "rust_swift_proof"));
        assert!(tools.iter().any(|t| t["name"] == "rust_swift_explain"));
    }

    #[test]
    fn test_mcp_tool_check_admitted() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "rust_swift_check",
                "arguments": {
                    "source": "pub fn add(a: i32, b: i32) -> i32 { a + b }"
                }
            }
        })
        .to_string();

        let resp = McpServer::handle_message(&req).expect("Expected response");
        let val: Value = serde_json::from_str(&resp).unwrap();
        let text = val["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"status\": \"admitted\""));
    }

    #[test]
    fn test_mcp_tool_check_rejected() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "rust_swift_check",
                "arguments": {
                    "source": "pub fn bad() { unsafe { } }"
                }
            }
        })
        .to_string();

        let resp = McpServer::handle_message(&req).expect("Expected response");
        let val: Value = serde_json::from_str(&resp).unwrap();
        let text = val["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"status\": \"rejected\""));
        assert!(text.contains("E0001_UNSAFE_REJECTED"));
    }

    #[test]
    fn test_mcp_tool_compile_direct() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "rust_swift_compile",
                "arguments": {
                    "source": "pub struct Counter { pub val: i64 }",
                    "package_name": "CounterModule"
                }
            }
        })
        .to_string();

        let resp = McpServer::handle_message(&req).expect("Expected response");
        let val: Value = serde_json::from_str(&resp).unwrap();
        let text = val["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"status\": \"success\""));
        assert!(text.contains("public struct Counter: Equatable, Hashable, Sendable"));
    }

    #[test]
    fn test_mcp_tool_explain() {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "rust_swift_explain",
                "arguments": {
                    "query": "E0001"
                }
            }
        })
        .to_string();

        let resp = McpServer::handle_message(&req).expect("Expected response");
        let val: Value = serde_json::from_str(&resp).unwrap();
        let text = val["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("E0001_UNSAFE_REJECTED"));
        assert!(text.contains("Agent Remedy"));
    }
}
