//! LedgerFlow deployment server: REST API, webhook, SaaS mode, and
//! standalone runtime.
//!
//! The server composes the core, protocol, facilitator, and wallet crates
//! into a runnable service. It owns:
//!
//! - `[saas]` mode (`standalone` | `saas`) with fail-fast configuration.
//! - REST endpoints for warrant issuance / revocation / audit / settlement.
//! - SaaS internal-header protocol (trusts only gateway-injected headers).
//! - Webhook event emission.

#![allow(missing_docs)]
#![allow(missing_debug_implementations)]

pub mod api;
pub mod config;
pub mod saas;
pub mod state;
pub mod webhook;

pub use crate::{
    api::{ApiError, ApiResponse, router},
    config::{SaasMode, ServerConfig},
    saas::{SaaSContext, SaasAuthError, SaasAuthExtractor, saas_auth_middleware},
    state::{AppState, NewAppState, ServerStateError},
    webhook::{WebhookEvent, WebhookSender},
};
