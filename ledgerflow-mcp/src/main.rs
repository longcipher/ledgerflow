use std::{sync::Arc, time::Duration};

use clap::Parser;
use color_eyre::Result;
use hpx::Client;
use tracing::info;
use ultrafast_mcp::{
    prelude::*, ListToolsRequest, ListToolsResponse, MCPError, MCPResult, ToolCall, ToolContent,
    ToolsCapability,
};
use x402_types::proto;

const DEFAULT_FACILITATOR_URL: &str = "http://127.0.0.1:3402";

#[derive(Parser, Debug)]
#[command(name = "ledgerflow-mcp")]
#[command(
    about = "MCP server exposing x402 v2 verify/settle/supported tools",
    long_about = None
)]
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

    /// Base URL for the facilitator service (must expose /supported, /verify, /settle).
    #[arg(long, default_value = DEFAULT_FACILITATOR_URL)]
    facilitator_url: String,
}

#[derive(Clone)]
struct X402ToolHandler {
    client: Client,
    facilitator_url: String,
}

impl X402ToolHandler {
    fn new(facilitator_url: String) -> Result<Self> {
        let facilitator_url = facilitator_url.trim_end_matches('/').to_string();
        if !facilitator_url.starts_with("http://") && !facilitator_url.starts_with("https://") {
            return Err(eyre::eyre!(
                "invalid facilitator_url '{facilitator_url}': expected http:// or https://"
            ));
        }

        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self {
            client,
            facilitator_url,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.facilitator_url, path.trim_start_matches('/'))
    }

    async fn decode_response<T: serde::de::DeserializeOwned>(
        response: hpx::Response,
        context: &str,
    ) -> MCPResult<T> {
        if response.status().is_success() {
            response.json::<T>().await.map_err(|e| {
                MCPError::internal_error(format!("{context}: failed to decode response JSON: {e}"))
            })
        } else {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string());
            Err(MCPError::internal_error(format!(
                "{context}: upstream facilitator status={status}, body={body}"
            )))
        }
    }

    async fn supported(&self) -> MCPResult<proto::SupportedResponse> {
        let response = self
            .client
            .get(self.endpoint("/supported"))
            .send()
            .await
            .map_err(|e| MCPError::internal_error(format!("GET /supported failed: {e}")))?;
        Self::decode_response(response, "GET /supported").await
    }

    async fn verify(&self, request: &proto::VerifyRequest) -> MCPResult<proto::VerifyResponse> {
        let response = self
            .client
            .post(self.endpoint("/verify"))
            .json(request)
            .send()
            .await
            .map_err(|e| MCPError::internal_error(format!("POST /verify failed: {e}")))?;
        Self::decode_response(response, "POST /verify").await
    }

    async fn settle(&self, request: &proto::SettleRequest) -> MCPResult<proto::SettleResponse> {
        let response = self
            .client
            .post(self.endpoint("/settle"))
            .json(request)
            .send()
            .await
            .map_err(|e| MCPError::internal_error(format!("POST /settle failed: {e}")))?;
        Self::decode_response(response, "POST /settle").await
    }
}

fn parse_verify_arguments(arguments: Option<serde_json::Value>) -> MCPResult<proto::VerifyRequest> {
    let value = arguments.ok_or_else(|| {
        MCPError::invalid_params(
            "x402_verify requires a JSON object with x402 v2 payload fields".to_string(),
        )
    })?;

    serde_json::from_value(value).map_err(|e| {
        MCPError::invalid_params(format!("invalid x402_verify arguments (x402 v2): {e}"))
    })
}

fn parse_settle_arguments(arguments: Option<serde_json::Value>) -> MCPResult<proto::SettleRequest> {
    let value = arguments.ok_or_else(|| {
        MCPError::invalid_params(
            "x402_settle requires a JSON object with x402 v2 payload fields".to_string(),
        )
    })?;

    serde_json::from_value(value).map_err(|e| {
        MCPError::invalid_params(format!("invalid x402_settle arguments (x402 v2): {e}"))
    })
}

fn to_tool_result_json(value: &serde_json::Value) -> MCPResult<ToolResult> {
    let text = serde_json::to_string(value)
        .map_err(|e| MCPError::internal_error(format!("failed to serialize tool response: {e}")))?;
    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: Some(false),
    })
}

#[async_trait::async_trait]
impl ToolHandler for X402ToolHandler {
    async fn handle_tool_call(&self, call: ToolCall) -> MCPResult<ToolResult> {
        match call.name.as_str() {
            "x402_supported" => {
                let supported = self.supported().await?;
                let value = serde_json::to_value(supported).map_err(|e| {
                    MCPError::internal_error(format!("failed to encode supported response: {e}"))
                })?;
                to_tool_result_json(&value)
            }
            "x402_verify" => {
                let req = parse_verify_arguments(call.arguments)?;
                let res = self.verify(&req).await?;
                let value = serde_json::to_value(res).map_err(|e| {
                    MCPError::internal_error(format!("failed to encode verify response: {e}"))
                })?;
                to_tool_result_json(&value)
            }
            "x402_settle" => {
                let req = parse_settle_arguments(call.arguments)?;
                let res = self.settle(&req).await?;
                let value = serde_json::to_value(res).map_err(|e| {
                    MCPError::internal_error(format!("failed to encode settle response: {e}"))
                })?;
                to_tool_result_json(&value)
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
                    description: "Fetch facilitator /supported (x402 v2 capabilities)".to_string(),
                    input_schema: serde_json::json!({"type":"object","properties":{}}),
                    output_schema: None,
                    annotations: None,
                },
                Tool {
                    name: "x402_verify".to_string(),
                    description: "Forward x402 v2 verify request to facilitator /verify"
                        .to_string(),
                    input_schema: serde_json::json!({
                        "type":"object",
                        "required":["x402Version","paymentPayload","paymentRequirements"],
                        "properties":{
                            "x402Version":{"type":"integer","const":2},
                            "paymentPayload":{"type":"object"},
                            "paymentRequirements":{"type":"object"}
                        }
                    }),
                    output_schema: None,
                    annotations: None,
                },
                Tool {
                    name: "x402_settle".to_string(),
                    description: "Forward x402 v2 settle request to facilitator /settle"
                        .to_string(),
                    input_schema: serde_json::json!({
                        "type":"object",
                        "required":["x402Version","paymentPayload","paymentRequirements"],
                        "properties":{
                            "x402Version":{"type":"integer","const":2},
                            "paymentPayload":{"type":"object"},
                            "paymentRequirements":{"type":"object"}
                        }
                    }),
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
    dotenvy::dotenv().ok();

    let args = Args::parse();
    let handler = X402ToolHandler::new(args.facilitator_url.clone())?;
    info!(facilitator_url = %args.facilitator_url, "configured facilitator upstream");

    let info = ServerInfo {
        name: "ledgerflow-mcp".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: Some("MCP server exposing x402 v2 verify/settle/supported".to_string()),
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
            use std::str::FromStr;

            let addr = std::net::SocketAddr::from_str(&format!("{}:{}", args.host, args.port))?;
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

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use axum::{
        routing::{get, post},
        Json, Router,
    };
    use x402_types::{chain::ChainId, proto::SupportedPaymentKind};

    use super::*;

    fn supported_response() -> proto::SupportedResponse {
        let mut signers = HashMap::new();
        signers.insert(
            ChainId::new("eip155", "84532"),
            vec!["0x00000000000000000000000000000000000000f1".to_string()],
        );
        proto::SupportedResponse {
            kinds: vec![SupportedPaymentKind {
                x402_version: 2,
                scheme: "exact".to_string(),
                network: "eip155:84532".to_string(),
                extra: Some(serde_json::json!({"assetTransferMethod":"eip3009"})),
            }],
            extensions: vec!["exact-eip3009".to_string()],
            signers,
        }
    }

    async fn mock_supported() -> Json<proto::SupportedResponse> {
        Json(supported_response())
    }

    async fn mock_verify(Json(_req): Json<proto::VerifyRequest>) -> Json<proto::VerifyResponse> {
        Json(proto::VerifyResponse(
            serde_json::json!({"isValid":true,"payer":"0x00000000000000000000000000000000000000aa"}),
        ))
    }

    async fn mock_settle(Json(_req): Json<proto::SettleRequest>) -> Json<proto::SettleResponse> {
        Json(proto::SettleResponse(serde_json::json!({
            "success":true,
            "payer":"0x00000000000000000000000000000000000000aa",
            "transaction":"0xfeed",
            "network":"eip155:84532"
        })))
    }

    async fn start_mock_facilitator() -> String {
        let app = Router::new()
            .route("/supported", get(mock_supported))
            .route("/verify", post(mock_verify))
            .route("/settle", post(mock_settle));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock facilitator");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock facilitator server failed");
        });
        format!("http://{addr}")
    }

    fn first_text(result: ToolResult) -> String {
        match result.content.first() {
            Some(ToolContent::Text { text }) => text.clone(),
            _ => panic!("expected text tool content"),
        }
    }

    fn verify_request_json() -> serde_json::Value {
        serde_json::json!({
            "x402Version": 2,
            "paymentPayload": {
                "x402Version": 2,
                "accepted": {
                    "scheme":"exact",
                    "network":"eip155:84532",
                    "amount":"1000000",
                    "payTo":"0x00000000000000000000000000000000000000f1",
                    "maxTimeoutSeconds":300,
                    "asset":"0x0000000000000000000000000000000000000010",
                    "extra":{"assetTransferMethod":"eip3009"}
                },
                "payload": {
                    "signature":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "authorization":{
                        "from":"0x00000000000000000000000000000000000000aa",
                        "to":"0x00000000000000000000000000000000000000f1",
                        "value":"1000000",
                        "validAfter":"1730000000",
                        "validBefore":"1999999999",
                        "nonce":"0x1111111111111111111111111111111111111111111111111111111111111111"
                    }
                },
                "resource": null
            },
            "paymentRequirements": {
                "scheme":"exact",
                "network":"eip155:84532",
                "amount":"1000000",
                "payTo":"0x00000000000000000000000000000000000000f1",
                "maxTimeoutSeconds":300,
                "asset":"0x0000000000000000000000000000000000000010",
                "extra":{"assetTransferMethod":"eip3009"}
            }
        })
    }

    #[tokio::test]
    async fn supported_tool_returns_v2_shape() {
        let base = start_mock_facilitator().await;
        let handler = Arc::new(X402ToolHandler::new(base).expect("create handler"));

        let result = handler
            .handle_tool_call(ToolCall {
                name: "x402_supported".to_string(),
                arguments: None,
            })
            .await
            .expect("supported tool call");

        let json: serde_json::Value =
            serde_json::from_str(&first_text(result)).expect("parse supported output");
        assert_eq!(json["kinds"][0]["x402Version"], 2);
        assert_eq!(json["kinds"][0]["network"], "eip155:84532");
        assert!(json["extensions"]
            .as_array()
            .expect("extensions array")
            .iter()
            .any(|entry| entry == "exact-eip3009"));
    }

    #[tokio::test]
    async fn verify_tool_forwards_v2_payload() {
        let base = start_mock_facilitator().await;
        let handler = Arc::new(X402ToolHandler::new(base).expect("create handler"));

        let result = handler
            .handle_tool_call(ToolCall {
                name: "x402_verify".to_string(),
                arguments: Some(verify_request_json()),
            })
            .await
            .expect("verify tool call");

        let json: serde_json::Value =
            serde_json::from_str(&first_text(result)).expect("parse verify output");
        assert_eq!(json["isValid"], true);
        assert_eq!(json["payer"], "0x00000000000000000000000000000000000000aa");
    }

    #[tokio::test]
    async fn verify_tool_requires_arguments() {
        let handler = Arc::new(
            X402ToolHandler::new("http://127.0.0.1:3402".to_string()).expect("create handler"),
        );

        let error = handler
            .handle_tool_call(ToolCall {
                name: "x402_verify".to_string(),
                arguments: None,
            })
            .await
            .expect_err("missing args should fail");
        let message = error.to_string();
        assert!(message.contains("Invalid parameters"));
        assert!(message.contains("requires a JSON object"));
    }
}
