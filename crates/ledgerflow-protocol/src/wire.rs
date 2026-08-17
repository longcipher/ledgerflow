//! Wire helpers: CBOR encode/decode with size limits, base64url, and carrier
//! size policy.

use serde::{de::DeserializeOwned, Serialize};

use crate::{error::ProtocolError, carrier::LedgerFlowCarrier};

/// CBOR-encodes a value with a size limit.
pub fn cbor_encode<T: Serialize>(value: &T, max_bytes: usize) -> Result<Vec<u8>, ProtocolError> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(value, &mut bytes)
        .map_err(|error| ProtocolError::Serialization(error.to_string()))?;
    if bytes.len() > max_bytes {
        return Err(ProtocolError::PayloadTooLarge { size: bytes.len(), max: max_bytes });
    }
    Ok(bytes)
}

/// CBOR-decodes a value with a size limit.
pub fn cbor_decode<T: DeserializeOwned>(bytes: &[u8], max_bytes: usize) -> Result<T, ProtocolError> {
    if bytes.len() > max_bytes {
        return Err(ProtocolError::PayloadTooLarge { size: bytes.len(), max: max_bytes });
    }
    ciborium::de::from_reader(bytes)
        .map_err(|error| ProtocolError::Deserialization(error.to_string()))
}

/// Base64url-encodes bytes (no padding, per URL-safe conventions).
pub fn base64url_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Base64url-decodes bytes (no padding).
pub fn base64url_decode(encoded: &str) -> Result<Vec<u8>, ProtocolError> {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|error| ProtocolError::InvalidBase64(error.to_string()))
}

/// Validates that a payload fits the given carrier's size budget.
///
/// Header carriers are size-constrained (single-node/digest references only);
/// body and in-process carriers are not.
pub const fn validate_carrier_fit(
    payload_bytes: usize,
    carrier: &LedgerFlowCarrier,
) -> Result<(), ProtocolError> {
    let max = carrier.max_bytes();
    if payload_bytes > max {
        return Err(ProtocolError::CarrierTooLarge { size: payload_bytes, max });
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn cbor_encode_respects_size_limit() {
        let bytes = cbor_encode(&42_u32, 64).expect("fits");
        assert_eq!(bytes, cbor_encode(&42_u32, 1024).expect("fits"));

        let error = cbor_encode(&42_u32, 0).expect_err("too small");
        assert!(matches!(error, ProtocolError::PayloadTooLarge { .. }));

        // Exactly-at-limit passes (`len > max` is strict).
        let exact = cbor_encode(&42_u32, bytes.len()).expect("exactly at limit");
        assert_eq!(exact, bytes);
    }

    #[test]
    fn cbor_decode_round_trips_and_limits() {
        let bytes = cbor_encode(&"hello".to_string(), 1024).expect("encode");
        let decoded: String = cbor_decode(&bytes, 1024).expect("decode");
        assert_eq!(decoded, "hello");

        let error = cbor_decode::<String>(&bytes, 1).expect_err("too small");
        assert!(matches!(error, ProtocolError::PayloadTooLarge { .. }));
    }

    #[test]
    fn base64url_round_trips() {
        let encoded = base64url_encode(b"hello world");
        assert_eq!(base64url_decode(&encoded).expect("decode"), b"hello world");
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));

        let error = base64url_decode("!!!not-base64!!!").expect_err("invalid");
        assert!(matches!(error, ProtocolError::InvalidBase64(_)));
    }

    #[test]
    fn validate_carrier_fit_bounds() {
        // Header limit = 2048; exactly-at-limit passes.
        assert!(validate_carrier_fit(2048, &LedgerFlowCarrier::HttpHeader).is_ok());
        assert!(validate_carrier_fit(2049, &LedgerFlowCarrier::HttpHeader).is_err());
        // Body carriers have no limit.
        assert!(validate_carrier_fit(usize::MAX, &LedgerFlowCarrier::HttpBody).is_ok());
    }
}
