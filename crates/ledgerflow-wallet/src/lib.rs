//! Wallet capability abstraction for LedgerFlow.
//!
//! LedgerFlow never depends on a specific wallet implementation. It depends
//! on the [`WalletSigner`] capability interface; any wallet that satisfies it
//! (embedded signer, local JSON-RPC daemon, WalletConnect v2 client, ...) can
//! be integrated without touching LedgerFlow's core.
//!
//! Provided adapters in v1:
//!
//! - [`embedded::EmbeddedSigner`]: an in-process signer over a raw `SigningKeyPair` (used by agents
//!   and control planes).
//! - [`local_rpc::LocalRpcSigner`]: a client for a local wallet daemon speaking JSON-RPC 2.0 over
//!   HTTP (loopback).
//! - [`server::EmbeddedWalletServer`]: an in-memory JSON-RPC 2.0 server over a [`WalletSigner`],
//!   plus (feature `http`) a loopback HTTP listener for end-to-end use with
//!   [`local_rpc::HttpJsonRpcTransport`].
//! - [`wc::WcV2Signer`] (feature `wc`): a WalletConnect v2 dApp client that pairs with any WC v2
//!   wallet (e.g. OneCipher) over a relay and routes signing requests through the session.

#![allow(missing_docs)]

pub mod approvals;
pub mod embedded;
pub mod error;
pub mod local_rpc;
pub mod server;
pub mod signer;
#[cfg(feature = "wc")]
pub mod wc;

#[cfg(feature = "http")]
pub use crate::local_rpc::HttpJsonRpcTransport;
#[cfg(feature = "http")]
pub use crate::server::LoopbackJsonRpcServer;
#[cfg(feature = "wc")]
pub use crate::wc::{WcV2Config, WcV2Signer};
pub use crate::{
    approvals::request_approval,
    embedded::EmbeddedSigner,
    error::WalletError,
    local_rpc::{
        JsonRpcError, JsonRpcRequest, JsonRpcResponse, LocalRpcConfig, LocalRpcSigner,
        MockJsonRpcTransport, RpcTransport,
    },
    server::EmbeddedWalletServer,
    signer::{
        SignDomain, SignPaymentRequest, SignRequest, SignResult, SignedPayment, WalletDescriptor,
        WalletSigner,
    },
};
