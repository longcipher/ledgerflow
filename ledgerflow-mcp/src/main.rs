use std::sync::Arc;

use clap::Parser;
use color_eyre::Result;
use tracing::info;
use ultrafast_mcp::{
    prelude::*, ListToolsRequest, ListToolsResponse, MCPError, MCPResult, ToolCall, ToolContent,
    ToolsCapability,
};
use x402_rs::{
    facilitator::Facilitator,
    facilitator_local::FacilitatorLocal,
    network::Network,
    provider_cache::ProviderCache,
    types::{
        Scheme, SettleRequest, SettleResponse, SupportedPaymentKind, VerifyRequest, VerifyResponse,
        X402Version,
    },
};

#[derive(Parser, Debug)]
#[command(name = "ledgerflow-mcp")]
#[command(about = "MCP server exposing x402 verify/settle/supported tools", long_about = None)]
pub struct Args {
    /// Run over stdio (default)
    #[arg(long)]
    stdio: bool,

    /// Run HTTP server instead of stdio
    #[arg(long)]
    http: bool,

    /// Host for HTTP server
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port for HTTP server
    #[arg(long, default_value_t = 8765)]
    port: u16,
}

#[derive(Clone)]
struct X402ToolHandler {
    facilitator: FacilitatorLocal,
}

#[async_trait::async_trait]
impl ToolHandler for X402ToolHandler {
    async fn handle_tool_call(&self, call: ToolCall) -> MCPResult<ToolResult> {
        match call.name.as_str() {
            "x402_supported" => {
                let kinds: Vec<SupportedPaymentKind> = Network::variants()
                    .iter()
                    .copied()
                    .map(|n| SupportedPaymentKind {
                        x402_version: X402Version::V1,
                        scheme: Scheme::Exact,
                        network: n,
                        extra: None,
                    })
                    .collect();
                let payload = serde_json::json!({ "supported": kinds });
                Ok(ToolResult {
                    content: vec![ToolContent::text(payload.to_string())],
                    is_error: Some(false),
                })
            }
            "x402_verify" => {
                let req_value = call.arguments.unwrap_or_default();
                let req: VerifyRequest = serde_json::from_value(req_value)
                    .map_err(|e| MCPError::invalid_params(format!("invalid arguments: {e}")))?;
                let res: VerifyResponse = self
                    .facilitator
                    .verify(&req)
                    .await
                    .map_err(|e| MCPError::internal_error(e.to_string()))?;
                Ok(ToolResult {
                    content: vec![ToolContent::text(
                        serde_json::to_string(&res).unwrap_or_else(|_| "{}".into()),
                    )],
                    is_error: Some(false),
                })
            }
            "x402_settle" => {
                let req_value = call.arguments.unwrap_or_default();
                let req: SettleRequest = serde_json::from_value(req_value)
                    .map_err(|e| MCPError::invalid_params(format!("invalid arguments: {e}")))?;
                let res: SettleResponse = self
                    .facilitator
                    .settle(&req)
                    .await
                    .map_err(|e| MCPError::internal_error(e.to_string()))?;
                Ok(ToolResult {
                    content: vec![ToolContent::text(
                        serde_json::to_string(&res).unwrap_or_else(|_| "{}".into()),
                    )],
                    is_error: Some(false),
                })
            }
            _ => Err(MCPError::method_not_found(format!(
                "tool '{}' not found",
                call.name
            ))),
        }
    }

    async fn list_tools(&self, _request: ListToolsRequest) -> MCPResult<ListToolsResponse> {
        Ok(ListToolsResponse {
            tools: vec![
                Tool {
                    name: "x402_supported".to_string(),
                    description: "List supported payment kinds (networks + schemes)".to_string(),
                    input_schema: serde_json::json!({"type":"object","properties":{}}),
                    output_schema: None,
                    annotations: None,
                },
                Tool {
                    name: "x402_verify".to_string(),
                    description: "Verify a payment intent using x402 Exact scheme".to_string(),
                    input_schema: serde_json::json!({"type":"object"}),
                    output_schema: None,
                    annotations: None,
                },
                Tool {
                    name: "x402_settle".to_string(),
                    description: "Settle a verified payment intent".to_string(),
                    input_schema: serde_json::json!({"type":"object"}),
                    output_schema: None,
                    annotations: None,
                },
            ],
            next_cursor: None,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    // Prepare providers/signers from env (shared with facilitator crate)
    dotenvy::dotenv().ok();
    let providers = ProviderCache::from_env()
        .await
        .map_err(|e| eyre::eyre!(format!("{e}")))?;
    let facilitator = FacilitatorLocal::new(providers);

    let handler = X402ToolHandler { facilitator };

    // Server info and capabilities
    let info = ServerInfo {
        name: "ledgerflow-mcp".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: Some("MCP server exposing x402 verify/settle/supported".to_string()),
        authors: None,
        homepage: None,
        license: None,
        repository: None,
    };

    let capabilities = ServerCapabilities {
        tools: Some(ToolsCapability {
            list_changed: Some(true),
        }),
        ..Default::default()
    };
    let server = UltraFastServer::new(info, capabilities).with_tool_handler(Arc::new(handler));

    if args.http {
        #[cfg(feature = "http")]
        {
            let addr = SocketAddr::from_str(&format!("{}:{}", args.host, args.port))?;
            info!(%addr, "Starting HTTP MCP server");
            server
                .run_streamable_http(addr.ip().to_string().as_str(), addr.port())
                .await?;
        }
        #[cfg(not(feature = "http"))]
        {
            info!("HTTP feature not enabled; falling back to stdio");
            server.run_stdio().await?;
        }
    } else {
        info!("Starting stdio MCP server");
        server.run_stdio().await?;
    }

    Ok(())
}
