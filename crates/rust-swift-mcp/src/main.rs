//! rust-swift-mcp binary entry point.
//! Runs the Model Context Protocol stdio transport server.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    rust_swift_mcp::McpServer::run_stdio()
}
