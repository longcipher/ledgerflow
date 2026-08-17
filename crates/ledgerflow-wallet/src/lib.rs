//! Wallet capability abstraction for LedgerFlow.
//!
//! LedgerFlow never depends on a specific wallet implementation. It depends
//! on the [`WalletSigner`] capability interface; any wallet that satisfies it
//! (embedded signer, local JSON-RPC daemon, WalletConnect v2 client, ...) can
//! be integrated without touching LedgerFlow's core.
//!
//! Provided adapters in v1:
//!
//! - [`embedded::EmbeddedSigner`]: an in-process signer over a raw
//!   `SigningKeyPair` (used by agents and control planes).
//! - [`local_rpc::LocalRpcSigner`]: a client for a local wallet daemon
//!   speaking JSON-RPC 2.0 over HTTP (loopback).

#![allow(missing_docs)]

pub mod approvals;
pub mod embedded;
pub mod error;
pub mod local_rpc;
pub mod signer;

pub use crate::{
    approvals::request_approval,
    embedded::EmbeddedSigner,
    error::WalletError,
    local_rpc::{
        JsonRpcError, JsonRpcRequest, JsonRpcResponse, LocalRpcConfig, LocalRpcSigner,
        MockJsonRpcTransport, RpcTransport,
    },
    signer::{
        SignDomain, SignPaymentRequest, SignRequest, SignResult, SignedPayment, WalletDescriptor,
        WalletSigner,
    },
};

#[cfg(feature = "http")]
pub use crate::local_rpc::HttpJsonRpcTransport;
