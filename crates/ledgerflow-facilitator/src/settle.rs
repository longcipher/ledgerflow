//! `/settle` orchestration with **atomic re-verification**.
//!
//! The verify step is only a pre-check. Settlement MUST re-verify revocation,
//! TTL, PoP freshness, and the payment cap atomically before touching any
//! rail, closing the verify→settle TOCTOU window (design §8.1).

use ledgerflow_core::{
    AuthorizationContext, AuthorizationError, PopProof, RevocationCheck, VerifiedAuthorization,
    WarrantChain, verify_freshness,
};

use crate::{
    outcome::SettlementOutcome,
    rails::RailAdapter,
    routing::RoutingError,
    subject::{PaymentSubjectResolver, ResolvedSubject},
};

/// Inputs to the settle orchestration.
#[derive(Clone, Debug)]
pub struct SettleRequest<'a> {
    /// The verified authorization from `/verify`.
    pub authorization: &'a VerifiedAuthorization,
    /// The presented chain (leaf re-verified for TTL/revocation).
    pub chain: &'a WarrantChain,
    /// The presented PoP.
    pub proof: &'a PopProof,
    /// The request context.
    pub context: &'a AuthorizationContext,
    /// Verification timestamp (unix milliseconds).
    pub now_ms: u64,
}

/// Settlement service that atomically re-verifies and routes to a rail.
#[derive(Clone)]
pub struct SettlementService<R, P, A> {
    pub revocation: R,
    pub resolver: P,
    pub adapters: Vec<A>,
}

impl<R, P, A> SettlementService<R, P, A>
where
    R: RevocationCheck,
    P: PaymentSubjectResolver,
    A: RailAdapter,
{
    /// Creates a new settlement service.
    #[must_use]
    pub const fn new(revocation: R, resolver: P, adapters: Vec<A>) -> Self {
        Self { revocation, resolver, adapters }
    }

    /// Settles a verified authorization after atomic re-verification.
    pub fn settle(&self, request: &SettleRequest<'_>) -> SettlementOutcome {
        // 1. Atomic re-verify: revocation + TTL + PoP freshness + amount cap.
        if let Err(error) = self.reverify(request) {
            return SettlementOutcome::failed(error.to_string());
        }

        // 2. Route to a rail.
        let resolved = match self.resolve(request.authorization) {
            Ok(subject) => subject,
            Err(error) => return SettlementOutcome::failed(error.to_string()),
        };
        let adapter = match self
            .adapters
            .iter()
            .find(|adapter| adapter.supports(&resolved))
        {
            Some(adapter) => adapter,
            None => {
                return SettlementOutcome::failed(RoutingError::NoCompatibleRail.to_string());
            }
        };

        // 3. Settle.
        match adapter.settle(request.authorization) {
            Ok(receipt) => SettlementOutcome::settled(receipt),
            Err(error) => SettlementOutcome::failed(error.to_string()),
        }
    }

    fn reverify(&self, request: &SettleRequest<'_>) -> Result<(), AuthorizationError> {
        let leaf = request.chain.leaf().ok_or(AuthorizationError::EmptyChain)?;
        // Revocation (online, persistent).
        self.revocation.verify(&leaf.id, &leaf.holder)?;
        // TTL.
        let now_secs = request.now_ms / 1000;
        if leaf.expires_at < now_secs {
            return Err(AuthorizationError::WarrantExpired { expires_at: leaf.expires_at });
        }
        // PoP freshness.
        verify_freshness(
            request.proof,
            request.now_ms,
            request.context.freshness_window_ms,
            request.context.clock_skew_ms,
        )?;
        // Amount cap.
        if request.authorization.amount > leaf.payment.max_per_charge {
            return Err(AuthorizationError::PaymentAmountExceeded {
                amount: request.authorization.amount,
                limit: leaf.payment.max_per_charge,
            });
        }
        Ok(())
    }

    fn resolve(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<ResolvedSubject, RoutingError> {
        Ok(self.resolver.resolve(authorization)?)
    }
}
