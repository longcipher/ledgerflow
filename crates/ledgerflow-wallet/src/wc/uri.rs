//! WalletConnect v2 pairing URI parser/serializer.
//!
//! Format: `wc:<topic>@<version>?relay-protocol=<p>&symKey=<hex>`

use std::fmt;

use crate::error::WalletError;

/// A parsed WC v2 pairing URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PairingUri {
    /// Pairing topic (hex).
    pub(crate) topic: String,
    /// Protocol version (always 2 for WC v2).
    pub(crate) version: u32,
    /// Relay protocol (e.g. `irn`).
    pub(crate) relay_protocol: Option<String>,
    /// Relay data (optional).
    pub(crate) relay_data: Option<String>,
    /// Pairing symmetric key (hex).
    pub(crate) sym_key: Option<String>,
    /// Optional methods hint.
    pub(crate) methods: Vec<String>,
    /// WalletConnect Cloud project ID (`projectId` query param). Optional for
    /// local relays; required for `relay.walletconnect.com`.
    pub(crate) project_id: Option<String>,
}

impl PairingUri {
    /// Parses a `wc:` URI string.
    pub(crate) fn parse(s: &str) -> Result<Self, WalletError> {
        let rest = s
            .strip_prefix("wc:")
            .ok_or_else(|| WalletError::InvalidPayload("missing 'wc:' scheme".to_string()))?;

        let (topic_version, query) = match rest.split_once('?') {
            Some((a, b)) => (a, Some(b)),
            None => (rest, None),
        };

        let (topic, version) = topic_version
            .split_once('@')
            .ok_or_else(|| WalletError::InvalidPayload("missing '@version'".to_string()))?;
        let version: u32 = version
            .parse()
            .map_err(|_| WalletError::InvalidPayload("version not a number".to_string()))?;

        let mut relay_protocol = None;
        let mut relay_data = None;
        let mut sym_key = None;
        let mut methods = Vec::new();
        let mut project_id = None;
        if let Some(q) = query {
            for pair in q.split('&') {
                let (k, v) = pair.split_once('=').ok_or_else(|| {
                    WalletError::InvalidPayload(format!("bad query pair: {pair}"))
                })?;
                match k {
                    "relay-protocol" => relay_protocol = Some(v.to_string()),
                    "relay-data" => relay_data = Some(v.to_string()),
                    "symKey" => sym_key = Some(v.to_string()),
                    "methods" => methods = v.split(',').map(String::from).collect(),
                    "projectId" => project_id = Some(v.to_string()),
                    _ => {} // forward-compat: ignore unknown keys
                }
            }
        }

        Ok(Self {
            topic: topic.to_string(),
            version,
            relay_protocol,
            relay_data,
            sym_key,
            methods,
            project_id,
        })
    }

    /// Constructs a new pairing URI from a topic and symmetric key.
    #[allow(dead_code)] // used by tests and future pairing-URI generation
    pub(crate) fn new(topic: impl Into<String>, sym_key: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            version: 2,
            // The official WC v2 default relay protocol is `irn`.
            relay_protocol: Some("irn".to_string()),
            relay_data: None,
            sym_key: Some(sym_key.into()),
            methods: Vec::new(),
            project_id: None,
        }
    }
}

impl fmt::Display for PairingUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "wc:{}@{}", self.topic, self.version)?;
        let mut first = true;
        let mut emit = |k: &str, v: &str, first: &mut bool| -> fmt::Result {
            if *first {
                write!(f, "?")?;
                *first = false;
            } else {
                write!(f, "&")?;
            }
            write!(f, "{k}={v}")
        };
        if let Some(p) = &self.relay_protocol {
            emit("relay-protocol", p, &mut first)?;
        }
        if let Some(d) = &self.relay_data {
            emit("relay-data", d, &mut first)?;
        }
        if let Some(k) = &self.sym_key {
            emit("symKey", k, &mut first)?;
        }
        if !self.methods.is_empty() {
            emit("methods", &self.methods.join(","), &mut first)?;
        }
        if let Some(pid) = &self.project_id {
            emit("projectId", pid, &mut first)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn parses_full_uri() {
        let uri =
            PairingUri::parse("wc:abc123@2?relay-protocol=irn&symKey=deadbeef&projectId=proj1")
                .expect("parse");
        assert_eq!(uri.topic, "abc123");
        assert_eq!(uri.version, 2);
        assert_eq!(uri.relay_protocol.as_deref(), Some("irn"));
        assert_eq!(uri.sym_key.as_deref(), Some("deadbeef"));
        assert_eq!(uri.project_id.as_deref(), Some("proj1"));
    }

    #[test]
    fn parses_minimal_uri() {
        let uri = PairingUri::parse("wc:abc@2").expect("parse");
        assert_eq!(uri.topic, "abc");
        assert_eq!(uri.version, 2);
        assert!(uri.sym_key.is_none());
    }

    #[test]
    fn rejects_missing_scheme() {
        assert!(PairingUri::parse("http://x").is_err());
    }

    #[test]
    fn rejects_missing_version() {
        assert!(PairingUri::parse("wc:abc").is_err());
    }

    #[test]
    fn display_round_trips() {
        let uri = PairingUri::new("topic1", "key1");
        let s = uri.to_string();
        assert_eq!(s, "wc:topic1@2?relay-protocol=irn&symKey=key1");
        let parsed = PairingUri::parse(&s).expect("parse");
        assert_eq!(parsed, uri);
    }
}
