//! MPP session state machine for streaming payments.
//!
//! MPP (Machine Payments Protocol) supports session-based streaming payments
//! where a warrant binds to a session lifetime. Revocation takes effect at
//! the next session tick, with the Facilitator actively closing the stream.
//!
//! This module implements the session state machine:
//!
//! - `SessionState`: the current state of a streaming session
//! - `SessionTick`: a discrete time step in the session
//! - `SessionEvent`: events that drive state transitions
//! - `SessionManager`: manages multiple sessions and their lifecycle

use std::collections::BTreeMap;

use ledgerflow_core::{RevocationCheck, RevocationDecision};
use thiserror::Error;

use crate::{outcome::SettlementOutcome, rails::RailAdapter, subject::PaymentSubjectResolver};

/// Default session tick duration (1 second).
pub const DEFAULT_TICK_DURATION_MS: u64 = 1_000;

/// Default session timeout (5 minutes).
pub const DEFAULT_SESSION_TIMEOUT_MS: u64 = 300_000;

/// Maximum number of ticks a session can be idle before it is closed.
pub const MAX_IDLE_TICKS: u64 = 300;

/// Session identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(pub String);

impl SessionId {
    /// Creates a new session id.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session state machine states.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    /// Session is active and accepting payments.
    Active,
    /// Session is closing (revocation detected, draining remaining ticks).
    Closing,
    /// Session is closed (no longer accepting payments).
    Closed,
}

/// A discrete time step in a streaming session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionTick {
    /// Tick sequence number (monotonically increasing).
    pub sequence: u64,
    /// Timestamp when the tick was created.
    pub timestamp_ms: u64,
}

/// Events that drive session state transitions.
#[derive(Clone, Debug)]
pub enum SessionEvent {
    /// A payment was received for this session.
    PaymentReceived { amount: u128, asset: String },
    /// A tick boundary was reached.
    Tick,
    /// Revocation was detected for the warrant or holder.
    Revoked,
    /// Session timeout was reached.
    Timeout,
    /// Session was explicitly closed.
    Closed,
}

/// Session configuration.
#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// Duration of each tick in milliseconds.
    pub tick_duration_ms: u64,
    /// Maximum session lifetime in milliseconds.
    pub timeout_ms: u64,
    /// Maximum number of idle ticks before closing.
    pub max_idle_ticks: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            tick_duration_ms: DEFAULT_TICK_DURATION_MS,
            timeout_ms: DEFAULT_SESSION_TIMEOUT_MS,
            max_idle_ticks: MAX_IDLE_TICKS,
        }
    }
}

/// A streaming payment session.
#[derive(Clone, Debug)]
pub struct Session {
    /// Session identifier.
    pub id: SessionId,
    /// Current state.
    pub state: SessionState,
    /// Current tick.
    pub current_tick: SessionTick,
    /// Total amount settled in this session.
    pub total_settled: u128,
    /// Asset being settled.
    pub asset: String,
    /// Configuration.
    pub config: SessionConfig,
    /// Timestamp when the session was created.
    pub created_at_ms: u64,
    /// Timestamp of the last activity.
    pub last_activity_ms: u64,
    /// Number of consecutive idle ticks.
    pub idle_ticks: u64,
    /// The warrant digest this session is bound to.
    pub warrant_digest: String,
    /// The holder's public key.
    pub holder: ledgerflow_core::SignerRef,
    /// The payment subject for settlement.
    pub payment_subject: ledgerflow_core::PaymentSubjectRef,
}

/// Session management errors.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session {0} is not active")]
    NotActive(SessionId),
    #[error("session {0} has expired")]
    Expired(SessionId),
    #[error("session {0} was revoked")]
    Revoked(SessionId),
    #[error("session {0} not found")]
    NotFound(SessionId),
    #[error("invalid session configuration: {0}")]
    InvalidConfig(String),
    #[error("subject resolution failed: {0}")]
    SubjectResolution(#[from] crate::subject::SubjectResolutionError),
    #[error("rail settlement failed: {0}")]
    RailSettlement(#[from] crate::rails::RailError),
}

/// Manages streaming payment sessions.
#[derive(Debug)]
pub struct SessionManager<R, P, A>
where
    R: RevocationCheck,
    P: PaymentSubjectResolver,
    A: RailAdapter,
{
    /// Active sessions.
    sessions: BTreeMap<SessionId, Session>,
    /// Revocation check for warrant status.
    revocation: R,
    /// Subject resolver for rail routing.
    resolver: P,
    /// Rail adapters for settlement.
    adapters: Vec<A>,
    /// Default session configuration.
    config: SessionConfig,
}

impl<R, P, A> SessionManager<R, P, A>
where
    R: RevocationCheck,
    P: PaymentSubjectResolver,
    A: RailAdapter,
{
    /// Creates a new session manager.
    #[must_use]
    pub const fn new(revocation: R, resolver: P, adapters: Vec<A>, config: SessionConfig) -> Self {
        Self { sessions: BTreeMap::new(), revocation, resolver, adapters, config }
    }

    /// Opens a new streaming session.
    pub fn open_session(
        &mut self,
        id: SessionId,
        asset: impl Into<String>,
        warrant_digest: impl Into<String>,
        holder: ledgerflow_core::SignerRef,
        payment_subject: ledgerflow_core::PaymentSubjectRef,
        now_ms: u64,
    ) -> Result<&Session, SessionError> {
        if self.sessions.contains_key(&id) {
            return Err(SessionError::InvalidConfig(format!("session {id} already exists")));
        }

        let session = Session {
            id: id.clone(),
            state: SessionState::Active,
            current_tick: SessionTick { sequence: 0, timestamp_ms: now_ms },
            total_settled: 0,
            asset: asset.into(),
            config: self.config.clone(),
            created_at_ms: now_ms,
            last_activity_ms: now_ms,
            idle_ticks: 0,
            warrant_digest: warrant_digest.into(),
            holder,
            payment_subject,
        };

        self.sessions.insert(id.clone(), session);
        self.sessions
            .get(&id)
            .ok_or_else(|| SessionError::InvalidConfig("session was just inserted".to_string()))
    }

    /// Processes a session event.
    pub fn process_event(
        &mut self,
        session_id: &SessionId,
        event: SessionEvent,
        now_ms: u64,
    ) -> Result<SessionState, SessionError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionError::NotFound(session_id.clone()))?;

        match event {
            SessionEvent::PaymentReceived { amount, asset } => {
                if session.state != SessionState::Active {
                    return Err(SessionError::NotActive(session_id.clone()));
                }
                session.total_settled += amount;
                session.asset = asset;
                session.last_activity_ms = now_ms;
                session.idle_ticks = 0;
            }
            SessionEvent::Tick => {
                // Validate tick duration to prevent tick manipulation
                let elapsed = now_ms.saturating_sub(session.current_tick.timestamp_ms);
                if elapsed < session.config.tick_duration_ms {
                    return Err(SessionError::InvalidConfig(format!(
                        "tick too fast: {}ms < {}ms",
                        elapsed, session.config.tick_duration_ms
                    )));
                }

                session.current_tick.sequence += 1;
                session.current_tick.timestamp_ms = now_ms;
                session.idle_ticks += 1;

                // If session is closing, close it on next tick
                if session.state == SessionState::Closing {
                    session.state = SessionState::Closed;
                    return Ok(SessionState::Closed);
                }

                // Check for timeout
                if now_ms - session.created_at_ms >= session.config.timeout_ms {
                    session.state = SessionState::Closed;
                    return Ok(SessionState::Closed);
                }

                // Check for idle timeout
                if session.idle_ticks >= session.config.max_idle_ticks {
                    session.state = SessionState::Closed;
                    return Ok(SessionState::Closed);
                }
            }
            SessionEvent::Revoked => {
                if session.state == SessionState::Active {
                    session.state = SessionState::Closing;
                }
            }
            SessionEvent::Timeout => {
                session.state = SessionState::Closed;
            }
            SessionEvent::Closed => {
                session.state = SessionState::Closed;
            }
        }

        Ok(session.state)
    }

    /// Checks if a session is active.
    #[must_use]
    pub fn is_active(&self, session_id: &SessionId) -> bool {
        self.sessions.get(session_id).is_some_and(|s| s.state == SessionState::Active)
    }

    /// Returns the current state of a session.
    #[must_use]
    pub fn get_state(&self, session_id: &SessionId) -> Option<SessionState> {
        self.sessions.get(session_id).map(|s| s.state)
    }

    /// Returns all active sessions.
    #[must_use]
    pub fn active_sessions(&self) -> Vec<&Session> {
        self.sessions.values().filter(|s| s.state == SessionState::Active).collect()
    }

    /// Checks if a warrant is revoked.
    #[must_use]
    pub fn is_revoked(&self, warrant_id: &[u8]) -> bool {
        !matches!(self.revocation.check_warrant(warrant_id), RevocationDecision::Ok)
    }

    /// Closes a session and returns the final settlement.
    pub fn close_session(
        &mut self,
        session_id: &SessionId,
        _now_ms: u64,
    ) -> Result<Option<SettlementOutcome>, SessionError> {
        // First, extract the data we need from the session
        let (total_settled, asset, warrant_digest, holder, payment_subject) = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SessionError::NotFound(session_id.clone()))?;

            if session.state == SessionState::Closed {
                return Ok(None);
            }

            session.state = SessionState::Closed;
            (
                session.total_settled,
                session.asset.clone(),
                session.warrant_digest.clone(),
                session.holder.clone(),
                session.payment_subject.clone(),
            )
        };

        // Build authorization from session data
        let authorization = Self::build_authorization_for_session(
            &asset,
            total_settled,
            &warrant_digest,
            &holder,
            &payment_subject,
        );
        let resolved = self.resolver.resolve(&authorization)?;
        let adapter = self
            .adapters
            .iter()
            .find(|a| a.supports(&resolved))
            .ok_or_else(|| SessionError::InvalidConfig("no compatible rail".to_string()))?;

        // Use the actual receipt from the adapter
        let receipt = adapter.settle(&authorization)?;

        Ok(Some(SettlementOutcome::settled(receipt)))
    }

    /// Builds an authorization for session settlement.
    fn build_authorization_for_session(
        asset: &str,
        total_settled: u128,
        warrant_digest: &str,
        holder: &ledgerflow_core::SignerRef,
        payment_subject: &ledgerflow_core::PaymentSubjectRef,
    ) -> ledgerflow_core::VerifiedAuthorization {
        ledgerflow_core::VerifiedAuthorization {
            merchant_id: "session".to_string(),
            tool_name: "streaming".to_string(),
            payment_subject: payment_subject.clone(),
            holder: holder.clone(),
            leaf_warrant: ledgerflow_core::Warrant {
                version: 1,
                id: [0; 16],
                holder: holder.clone(),
                issuer: holder.clone(),
                issued_at: 0,
                expires_at: u64::MAX,
                depth: 0,
                max_depth: 0,
                parent_hash: None,
                merchant: ledgerflow_core::MerchantConstraint::default(),
                resource: ledgerflow_core::ResourceConstraint::default(),
                payment: ledgerflow_core::PaymentConstraint::new(u128::MAX),
                tool: None,
                approval_gates: std::collections::BTreeMap::new(),
                required_approvers: Vec::new(),
                min_approvals: 0,
                extensions: std::collections::BTreeMap::new(),
                signature: ledgerflow_core::SignatureEnvelope {
                    alg: ledgerflow_core::SigningAlgorithm::Ed25519,
                    value: vec![0; 64],
                },
            },
            root_warrant: ledgerflow_core::Warrant {
                version: 1,
                id: [0; 16],
                holder: holder.clone(),
                issuer: holder.clone(),
                issued_at: 0,
                expires_at: u64::MAX,
                depth: 0,
                max_depth: 0,
                parent_hash: None,
                merchant: ledgerflow_core::MerchantConstraint::default(),
                resource: ledgerflow_core::ResourceConstraint::default(),
                payment: ledgerflow_core::PaymentConstraint::new(u128::MAX),
                tool: None,
                approval_gates: std::collections::BTreeMap::new(),
                required_approvers: Vec::new(),
                min_approvals: 0,
                extensions: std::collections::BTreeMap::new(),
                signature: ledgerflow_core::SignatureEnvelope {
                    alg: ledgerflow_core::SigningAlgorithm::Ed25519,
                    value: vec![0; 64],
                },
            },
            chain_len: 1,
            amount: total_settled,
            asset: asset.to_string(),
            scheme: "session".to_string(),
            payee_id: "session".to_string(),
            rail: ledgerflow_core::PaymentRail::Onchain,
            challenge_id: "session".to_string(),
            request_hash: "session".to_string(),
            accepted_hash: "session".to_string(),
            warrant_digest: warrant_digest.to_string(),
        }
    }

    /// Returns the number of active sessions.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.sessions.values().filter(|s| s.state == SessionState::Active).count()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use ledgerflow_core::InMemoryRevocationCheck;

    use super::*;
    use crate::{rails::evm::EvmRailAdapter, subject::DefaultSubjectResolver};

    fn test_config() -> SessionConfig {
        SessionConfig { tick_duration_ms: 100, timeout_ms: 1_000, max_idle_ticks: 5 }
    }

    #[test]
    fn session_lifecycle() {
        let mut manager = SessionManager::new(
            InMemoryRevocationCheck::new(),
            DefaultSubjectResolver,
            vec![EvmRailAdapter],
            test_config(),
        );

        let session_id = SessionId::new("test-session");
        let now = 1_000_000;
        let holder = ledgerflow_core::SignerRef::new(
            ledgerflow_core::SigningAlgorithm::Ed25519,
            vec![0x42; 32],
        );
        let payment_subject = ledgerflow_core::PaymentSubjectRef::new(
            ledgerflow_core::PaymentSubjectKind::Caip10,
            "caip10:eip155:8453:0xabc123",
        );

        // Open session
        let session = manager
            .open_session(
                session_id.clone(),
                "USDC",
                "warrant-digest-1",
                holder,
                payment_subject,
                now,
            )
            .expect("open session");
        assert_eq!(session.state, SessionState::Active);
        assert!(manager.is_active(&session_id));

        // Process payment
        let state = manager
            .process_event(
                &session_id,
                SessionEvent::PaymentReceived { amount: 100, asset: "USDC".to_string() },
                now + 100,
            )
            .expect("process payment");
        assert_eq!(state, SessionState::Active);

        // Process tick
        let state = manager
            .process_event(&session_id, SessionEvent::Tick, now + 200)
            .expect("process tick");
        assert_eq!(state, SessionState::Active);

        // Close session
        let outcome = manager.close_session(&session_id, now + 300).expect("close session");
        assert!(outcome.is_some());
        assert!(!manager.is_active(&session_id));
    }

    #[test]
    fn session_timeout() {
        let mut manager = SessionManager::new(
            InMemoryRevocationCheck::new(),
            DefaultSubjectResolver,
            vec![EvmRailAdapter],
            test_config(),
        );

        let session_id = SessionId::new("timeout-session");
        let now = 1_000_000;
        let holder = ledgerflow_core::SignerRef::new(
            ledgerflow_core::SigningAlgorithm::Ed25519,
            vec![0x42; 32],
        );
        let payment_subject = ledgerflow_core::PaymentSubjectRef::new(
            ledgerflow_core::PaymentSubjectKind::Caip10,
            "caip10:eip155:8453:0xabc123",
        );

        manager
            .open_session(
                session_id.clone(),
                "USDC",
                "warrant-digest-1",
                holder,
                payment_subject,
                now,
            )
            .expect("open session");

        // Advance time beyond timeout
        let state = manager
            .process_event(&session_id, SessionEvent::Tick, now + 2_000)
            .expect("process tick");
        assert_eq!(state, SessionState::Closed);
    }

    #[test]
    fn session_not_found() {
        let mut manager = SessionManager::new(
            InMemoryRevocationCheck::new(),
            DefaultSubjectResolver,
            vec![EvmRailAdapter],
            test_config(),
        );

        let result =
            manager.process_event(&SessionId::new("nonexistent"), SessionEvent::Tick, 1_000_000);
        assert!(matches!(result, Err(SessionError::NotFound(_))));
    }
}
