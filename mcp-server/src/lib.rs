//! `anthro-bridge-mcp-server` — a standalone or embedded MCP server exposing
//! a single `plan` tool.
//!
//! The library target exists so integration tests under `tests/` and the main
//! Poverello GUI crate (`gui/src-tauri`) can run the MCP server without
//! duplicating code.

pub mod mcp;
pub mod provider;

use std::error::Error;
use tracing_subscriber::EnvFilter;
use rmcp::{transport::stdio, ServiceExt};

use crate::mcp::PlannerTool;
use crate::provider::adapter::DynamicBridgeProvider;

/// Runs the Poverello MCP server over stdio until termination.
///
/// Diagnostic logging is written to `stderr` to ensure `stdout` is dedicated
/// strictly to MCP JSON-RPC protocol transport.
pub async fn run_stdio_mcp_server() -> Result<(), Box<dyn Error + Send + Sync>> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();

    let provider = DynamicBridgeProvider::new();
    let tool = PlannerTool::new(provider);

    tracing::info!("starting Poverello MCP server over stdio");

    let service = tool.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}
