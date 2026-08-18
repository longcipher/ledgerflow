//! Transport carriers for LedgerFlow authorization data.
//!
//! v1 rules (design §7.1 / §7.2):
//!
//! - **Header carriers** (MPP `WWW-Authenticate`/`Authorization`, x402 HTTP headers) are limited to
//!   single-warrant or digest references — full chains never fit in headers.
//! - **Body carriers** (402 response body, MCP `_meta`, A2A params) carry the full inline chain.
//! - **In-process** carriers have no size limit.

use crate::error::ProtocolError;

/// Maximum bytes a header carrier may transport.
pub const MAX_HEADER_CBOR_BYTES: usize = 2048;

/// A transport carrier for LedgerFlow data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LedgerFlowCarrier {
    /// HTTP header (single-node / digest reference only).
    HttpHeader,
    /// HTTP body (full chain).
    HttpBody,
    /// MCP `_meta` parameter.
    McpMeta,
    /// A2A parameter.
    A2AParam,
    /// In-process (no limit).
    InProcess,
}

impl LedgerFlowCarrier {
    /// Maximum byte budget for this carrier.
    #[must_use]
    pub const fn max_bytes(self) -> usize {
        match self {
            Self::HttpHeader => MAX_HEADER_CBOR_BYTES,
            Self::HttpBody | Self::McpMeta | Self::A2AParam | Self::InProcess => usize::MAX,
        }
    }

    /// Validates that `payload_bytes` fits this carrier.
    pub const fn validate(self, payload_bytes: usize) -> Result<(), ProtocolError> {
        let max = self.max_bytes();
        if payload_bytes > max {
            return Err(ProtocolError::CarrierTooLarge { size: payload_bytes, max });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn header_carrier_has_bounded_budget() {
        assert_eq!(LedgerFlowCarrier::HttpHeader.max_bytes(), MAX_HEADER_CBOR_BYTES);
        assert_eq!(LedgerFlowCarrier::HttpBody.max_bytes(), usize::MAX);
        assert_eq!(LedgerFlowCarrier::McpMeta.max_bytes(), usize::MAX);
        assert_eq!(LedgerFlowCarrier::A2AParam.max_bytes(), usize::MAX);
        assert_eq!(LedgerFlowCarrier::InProcess.max_bytes(), usize::MAX);
    }

    #[test]
    fn header_validate_rejects_oversized_payloads() {
        assert!(LedgerFlowCarrier::HttpHeader.validate(MAX_HEADER_CBOR_BYTES).is_ok());
        assert!(LedgerFlowCarrier::HttpHeader.validate(MAX_HEADER_CBOR_BYTES + 1).is_err());
        let error = LedgerFlowCarrier::HttpHeader.validate(9_999).expect_err("too large");
        assert!(matches!(error, ProtocolError::CarrierTooLarge { .. }));
    }

    #[test]
    fn body_carriers_accept_any_size() {
        assert!(LedgerFlowCarrier::HttpBody.validate(10 * 1024 * 1024).is_ok());
        assert!(LedgerFlowCarrier::McpMeta.validate(usize::MAX).is_ok());
    }

    #[test]
    fn carriers_are_distinct_and_comparable() {
        let carriers = [
            LedgerFlowCarrier::HttpHeader,
            LedgerFlowCarrier::HttpBody,
            LedgerFlowCarrier::McpMeta,
            LedgerFlowCarrier::A2AParam,
            LedgerFlowCarrier::InProcess,
        ];
        for (i, first) in carriers.iter().enumerate() {
            for (j, second) in carriers.iter().enumerate() {
                assert_eq!(first == second, i == j);
            }
        }
    }
}
