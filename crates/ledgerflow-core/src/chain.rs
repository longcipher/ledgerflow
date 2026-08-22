//! Delegation-chain verification (invariants I1-I7).
//!
//! A warrant chain is an ordered list of warrants from root to leaf. Each
//! child warrant is issued by the parent's holder (I1), is exactly one level
//! deeper (I2), expires no later than its parent (I3), is cryptographically
//! linked via `parent_hash` (I5), and satisfies monotonic attenuation over
//! the decidable fields (I4 static parts, I7 amount).
//!
//! Capability attenuation is enforced with **runtime conjunction** (I4): a
//! request must satisfy the constraints of *every* node in the chain. This
//! avoids undecidable static subset checking of URL patterns.
//!
//! A presented chain must not contain the same warrant id twice: duplicate
//! ids would allow re-arranging nodes (or a same-node-cycle) to confuse the
//! chain's depth accounting. This is checked before any linkage work.

use std::collections::HashSet;

use crate::{
    agent_identity::IdentityResolver,
    constraint::{AuthorizationContext, Verify},
    error::{AuthorizationError, Result},
    pop::{PopProof, verify_freshness},
    trust::TrustedIssuers,
    warrant::{Warrant, sha256_prefixed},
};

/// An ordered warrant chain from root (index 0) to leaf (last index).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WarrantChain {
    /// Root-first ordering.
    pub warrants: Vec<Warrant>,
}

impl WarrantChain {
    /// Creates a chain with a single (root) warrant.
    #[must_use]
    pub fn single(warrant: Warrant) -> Self {
        Self { warrants: vec![warrant] }
    }

    /// Returns the leaf (presenting) warrant.
    #[must_use]
    pub fn leaf(&self) -> Option<&Warrant> {
        self.warrants.last()
    }

    /// Returns the root warrant.
    #[must_use]
    pub fn root(&self) -> Option<&Warrant> {
        self.warrants.first()
    }

    /// Number of warrants in the chain.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.warrants.len()
    }

    /// Returns `true` when the chain is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.warrants.is_empty()
    }

    /// Pushes a warrant onto the chain (callers should verify before use).
    pub fn push(&mut self, warrant: Warrant) {
        self.warrants.push(warrant);
    }

    /// Verifies the full chain: cryptographic linkage, monotonic attenuation,
    /// trust anchor, PoP, freshness, and runtime-conjunction constraints.
    ///
    /// Online checks (revocation, budget accounting) are deliberately NOT
    /// performed here; callers must run them at settlement time (see the
    /// Facilitator design).
    pub fn verify(
        &self,
        trusted: &TrustedIssuers,
        proof: &PopProof,
        context: &AuthorizationContext,
    ) -> Result<VerifiedChainAuthorization> {
        verify_chain(self, trusted, proof, context)
    }
}

/// Output of a successful chain verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedChainAuthorization {
    /// The leaf warrant that was presented.
    pub leaf: Warrant,
    /// The root warrant (trust anchor subject).
    pub root: Warrant,
    /// The verified PoP.
    pub proof: PopProof,
    /// The chain length (number of warrants).
    pub chain_len: usize,
}

/// Verifies the chain invariants in order.
pub fn verify_chain(
    chain: &WarrantChain,
    trusted: &TrustedIssuers,
    proof: &PopProof,
    context: &AuthorizationContext,
) -> Result<VerifiedChainAuthorization> {
    verify_chain_with_resolver(chain, trusted, None, proof, context)
}

/// Verifies the chain invariants with an optional EIP-8004 identity resolver
/// for anchored trust entries (see [`crate::trust::TrustedIssuer::anchored`]).
pub fn verify_chain_with_resolver(
    chain: &WarrantChain,
    trusted: &TrustedIssuers,
    resolver: Option<&dyn IdentityResolver>,
    proof: &PopProof,
    context: &AuthorizationContext,
) -> Result<VerifiedChainAuthorization> {
    if chain.is_empty() {
        return Err(AuthorizationError::EmptyChain);
    }

    let root = &chain.warrants[0];
    // Trust anchor: the root issuer must be trusted (static keys or resolved
    // EIP-8004 anchors).
    trusted.verify_root_with_resolver(root, resolver)?;

    // Cycle detection: the same warrant id must not appear twice. Duplicate
    // ids would permit re-ordering nodes (or a self-referencing cycle) that
    // bypasses the depth accounting below.
    let mut seen = HashSet::with_capacity(chain.len());
    for node in &chain.warrants {
        if !seen.insert(node.id.as_slice()) {
            return Err(AuthorizationError::DuplicateWarrantInChain { warrant_id: node.id_hex() });
        }
    }

    // Per-node checks: envelope signature, time bounds, runtime-conjunction
    // constraints, and delegation capability on non-leaf nodes.
    for (index, node) in chain.warrants.iter().enumerate() {
        if node.version != crate::warrant::WARRANT_VERSION_V1 {
            return Err(AuthorizationError::UnsupportedVersion(node.version));
        }
        if !node.verify_signature() {
            return Err(AuthorizationError::InvalidWarrantSignature);
        }
        if node.issued_at > context.now_ms / 1000 {
            return Err(AuthorizationError::WarrantNotYetValid { issued_at: node.issued_at });
        }
        if node.expires_at < context.now_ms / 1000 {
            return Err(AuthorizationError::WarrantExpired { expires_at: node.expires_at });
        }
        // Runtime conjunction: every node must satisfy the context constraints.
        verify_node_constraints(node, context)?;

        // Non-leaf nodes must permit delegation.
        if index + 1 < chain.warrants.len() && node.max_depth == 0 {
            return Err(AuthorizationError::DelegationNotAllowed);
        }
    }

    // Linkage and monotonic attenuation between consecutive nodes.
    for pair in chain.warrants.windows(2) {
        let (parent, child) = (&pair[0], &pair[1]);
        verify_link(parent, child)?;
    }

    // Chain depth ceiling.
    let depth = (chain.len() - 1) as u8;
    if depth > crate::warrant::MAX_DELEGATION_DEPTH {
        return Err(AuthorizationError::DelegationDepthExceeded {
            presented: depth,
            allowed: crate::warrant::MAX_DELEGATION_DEPTH,
        });
    }

    let leaf = &chain.warrants[chain.warrants.len() - 1];

    // PoP must be presented by the leaf holder.
    if proof.tuple.warrant_id != leaf.id {
        return Err(AuthorizationError::WarrantDigestMismatch);
    }
    if !proof.verify_signature(&leaf.holder) {
        return Err(AuthorizationError::InvalidProofSignature);
    }
    if proof.tuple.challenge_id != context.challenge_id {
        return Err(AuthorizationError::ChallengeMismatch);
    }
    if proof.tuple.request_hash != context.request_hash {
        return Err(AuthorizationError::RequestHashMismatch);
    }
    if proof.tuple.accepted_hash != context.accepted_hash {
        return Err(AuthorizationError::AcceptedHashMismatch);
    }
    if proof.signer_key != leaf.holder.public_key {
        return Err(AuthorizationError::SignerMismatch);
    }
    verify_freshness(proof, context.now_ms, context.freshness_window_ms, context.clock_skew_ms)?;

    Ok(VerifiedChainAuthorization {
        leaf: leaf.clone(),
        root: root.clone(),
        proof: proof.clone(),
        chain_len: chain.len(),
    })
}

/// Verifies the pairwise linkage invariants between parent and child.
pub fn verify_link(parent: &Warrant, child: &Warrant) -> Result<()> {
    // I1: delegation authority.
    if child.issuer != parent.holder {
        return Err(AuthorizationError::DelegationAuthorityMismatch);
    }
    // I2: depth monotonicity.
    if child.depth != parent.depth + 1 {
        return Err(AuthorizationError::DepthMismatch {
            expected: parent.depth + 1,
            actual: child.depth,
        });
    }
    // I3: TTL monotonicity.
    if child.expires_at > parent.expires_at {
        return Err(AuthorizationError::TtlMonotonicityViolation);
    }
    // I5: cryptographic linkage (domain-separated parent payload hash).
    let expected_parent_hash = sha256_prefixed(parent.payload_bytes());
    let Some(actual) = &child.parent_hash else {
        return Err(AuthorizationError::MissingParentHash);
    };
    let actual = String::from_utf8_lossy(actual);
    if actual != expected_parent_hash {
        return Err(AuthorizationError::ParentHashMismatch);
    }
    // I7: amount monotonicity (child cap <= parent cap).
    if child.payment.max_per_charge > parent.payment.max_per_charge {
        return Err(AuthorizationError::AmountMonotonicityViolation);
    }
    // I4 static: child depth within parent's max_depth.
    if child.depth as u8 > parent.max_depth {
        return Err(AuthorizationError::DelegationDepthExceeded {
            presented: child.depth as u8,
            allowed: parent.max_depth,
        });
    }
    Ok(())
}

/// Evaluates the runtime-conjunction constraints of a single node.
fn verify_node_constraints(node: &Warrant, context: &AuthorizationContext) -> Result<()> {
    node.merchant.verify(context)?;
    node.resource.verify(context)?;
    if let Some(tool) = &node.tool {
        tool.verify(context)?;
    }
    node.payment.verify(context)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::{
        TrustedIssuer, TrustedIssuers,
        constraint::{
            AuthorizationContext, MerchantConstraint, PaymentConstraint, ResourceConstraint,
        },
        pop::PopProof,
        proof_builder::ProofBuilder,
        typestate::WarrantBuilder,
        warrant::{
            PaymentRail, PaymentSubjectKind, PaymentSubjectRef, SignerRef, SigningKeyPair,
            sha256_prefixed,
        },
    };

    fn issuer_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x11; 32])
    }

    fn holder_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x22; 32])
    }

    fn delegate_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x33; 32])
    }

    fn fixed_id(tag: &str) -> [u8; 16] {
        let mut id = [0_u8; 16];
        for (i, b) in tag.bytes().take(16).enumerate() {
            id[i] = b;
        }
        id
    }

    fn merchant() -> MerchantConstraint {
        MerchantConstraint::with_ids(vec!["merchant-a".to_string()])
    }

    fn resource() -> ResourceConstraint {
        ResourceConstraint {
            http_methods: vec!["POST".to_string()],
            path_prefixes: vec!["/pay".to_string()],
        }
    }

    fn payment(cap: u128) -> PaymentConstraint {
        PaymentConstraint::new(cap)
    }

    fn root_warrant(now_ms: u64, ttl_secs: u64, max_depth: u8) -> Warrant {
        WarrantBuilder::new(now_ms)
            .warrant_id(fixed_id("root-00000000000"))
            .ttl_secs(ttl_secs)
            .max_depth(max_depth)
            .issuer(issuer_keys().signer_ref())
            .holder(holder_keys().signer_ref())
            .merchant(merchant())
            .resource(resource())
            .payment(payment(1_000))
            .sign_with(&issuer_keys(), [0_u8; 8])
    }

    fn trusted() -> TrustedIssuers {
        let mut set = TrustedIssuers::new();
        set.add(TrustedIssuer::new("issuer-1".to_string(), issuer_keys().signer_ref()));
        set
    }

    fn context(now_ms: u64, holder: &SignerRef) -> AuthorizationContext {
        let request_hash = sha256_prefixed("POST\nmerchant-a.example\n/pay\nsha256:body");
        let accepted_hash = sha256_prefixed("exact:USDC:100:merchant-a");
        AuthorizationContext {
            merchant_id: "merchant-a".to_string(),
            merchant_host: "merchant-a.example".to_string(),
            tool_name: "web-search".to_string(),
            model_provider: String::new(),
            action_label: String::new(),
            http_method: "POST".to_string(),
            path_and_query: "/pay".to_string(),
            selected_amount: 100,
            asset: "USDC".to_string(),
            asset_network: Some("base".to_string()),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash,
            accepted_hash,
            now_ms,
            freshness_window_ms: 60_000,
            clock_skew_ms: 30_000,
            payment_subject: PaymentSubjectRef::new(
                PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            ),
            presenter: holder.clone(),
            human_present: false,
        }
    }

    fn proof_for(
        warrant: &Warrant,
        context: &AuthorizationContext,
        signer: &SigningKeyPair,
    ) -> PopProof {
        ProofBuilder::new()
            .warrant_id(warrant.id.clone())
            .challenge_id(context.challenge_id.clone())
            .method(context.http_method.clone())
            .uri(format!("{}{}", context.merchant_host, context.path_and_query))
            .request_hash(context.request_hash.clone())
            .accepted_hash(context.accepted_hash.clone())
            .payment_payload_digest(sha256_prefixed("x402-payload"))
            .nonce("nonce-1".to_string())
            .created_at_ms(context.now_ms)
            .sign_with(signer)
    }

    // ---------------------------------------------------------------------
    // WarrantChain container methods
    // ---------------------------------------------------------------------

    #[test]
    fn chain_container_methods_behave() {
        let warrant = root_warrant(2_000, 60, 3);
        let mut chain = WarrantChain::single(warrant.clone());
        assert_eq!(chain.len(), 1);
        assert!(!chain.is_empty());
        assert_eq!(chain.root(), Some(&warrant));
        assert_eq!(chain.leaf(), Some(&warrant));

        let child = crate::typestate::DelegatedWarrantBuilder::from(warrant).issue_to(
            delegate_keys().signer_ref(),
            &holder_keys(),
            2_000,
            [0_u8; 8],
        );
        chain.push(child.clone());
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.leaf(), Some(&child));
        assert_eq!(
            chain.root().expect("root").holder.public_key,
            holder_keys().public_key_bytes().to_vec()
        );
        assert_ne!(chain, WarrantChain::default());
    }

    #[test]
    fn empty_chain_is_rejected() {
        let chain = WarrantChain::default();
        assert!(chain.is_empty());
        let proof = proof_for(
            &root_warrant(2_000, 60, 3),
            &context(2_000, &holder_keys().signer_ref()),
            &holder_keys(),
        );
        let error =
            verify_chain(&chain, &trusted(), &proof, &context(2_000, &holder_keys().signer_ref()))
                .expect_err("empty");
        assert_eq!(error, AuthorizationError::EmptyChain);
    }

    // ---------------------------------------------------------------------
    // verify_chain timing and depth branches
    // ---------------------------------------------------------------------

    #[test]
    fn not_yet_valid_warrant_is_rejected() {
        // Warrant issued at t=2s; verify at t=1s => not yet valid.
        let warrant = root_warrant(2_000, 60, 3);
        let ctx = context(1_000, &holder_keys().signer_ref());
        let proof = proof_for(&warrant, &ctx, &holder_keys());
        let error = verify_chain(&WarrantChain::single(warrant), &trusted(), &proof, &ctx)
            .expect_err("not yet valid");
        assert!(matches!(error, AuthorizationError::WarrantNotYetValid { .. }));
    }

    #[test]
    fn expired_warrant_is_rejected() {
        // Warrant issued at t=2s with ttl 60s => expires at t=62s; verify at t=63s.
        let warrant = root_warrant(2_000, 60, 3);
        let ctx = context(63_000, &holder_keys().signer_ref());
        let proof = proof_for(&warrant, &ctx, &holder_keys());
        let error = verify_chain(&WarrantChain::single(warrant), &trusted(), &proof, &ctx)
            .expect_err("expired");
        assert!(matches!(error, AuthorizationError::WarrantExpired { .. }));
    }

    #[test]
    fn non_delegatable_root_with_child_is_rejected() {
        // max_depth = 0 on root means no child may follow.
        let root = root_warrant(2_000, 60, 0);
        let child = crate::typestate::DelegatedWarrantBuilder::from(root.clone()).issue_to(
            delegate_keys().signer_ref(),
            &holder_keys(),
            2_000,
            [0_u8; 8],
        );
        let chain = WarrantChain { warrants: vec![root, child] };
        let leaf = chain.leaf().expect("leaf");
        let ctx = context(2_000, &delegate_keys().signer_ref());
        let proof = proof_for(leaf, &ctx, &delegate_keys());
        let error = verify_chain(&chain, &trusted(), &proof, &ctx).expect_err("no delegation");
        assert_eq!(error, AuthorizationError::DelegationNotAllowed);
    }

    // ---------------------------------------------------------------------
    // verify_link invariants (I1-I7)
    // ---------------------------------------------------------------------

    fn child_warrant() -> Warrant {
        crate::typestate::DelegatedWarrantBuilder::from(root_warrant(2_000, 60, 3)).issue_to(
            delegate_keys().signer_ref(),
            &holder_keys(),
            2_000,
            [0_u8; 8],
        )
    }

    #[test]
    fn link_rejects_issuer_mismatch_i1() {
        let parent = root_warrant(2_000, 60, 3);
        let mut child = child_warrant();
        child.issuer = SigningKeyPair::from_bytes(&[0x44; 32]).signer_ref();
        let error = verify_link(&parent, &child).expect_err("I1");
        assert_eq!(error, AuthorizationError::DelegationAuthorityMismatch);
    }

    #[test]
    fn link_rejects_depth_mismatch_i2() {
        let parent = root_warrant(2_000, 60, 3);
        let mut child = child_warrant();
        child.depth = parent.depth + 2;
        let error = verify_link(&parent, &child).expect_err("I2");
        // The error payload must report the exact expected depth.
        assert_eq!(
            error,
            AuthorizationError::DepthMismatch { expected: parent.depth + 1, actual: child.depth }
        );
    }

    #[test]
    fn link_rejects_ttl_violation_i3() {
        let parent = root_warrant(2_000, 60, 3);
        let mut child = child_warrant();
        child.expires_at = parent.expires_at + 100;
        let error = verify_link(&parent, &child).expect_err("I3");
        assert_eq!(error, AuthorizationError::TtlMonotonicityViolation);
    }

    #[test]
    fn link_rejects_missing_parent_hash_i5() {
        let parent = root_warrant(2_000, 60, 3);
        let mut child = child_warrant();
        child.parent_hash = None;
        let error = verify_link(&parent, &child).expect_err("I5 missing");
        assert_eq!(error, AuthorizationError::MissingParentHash);
    }

    #[test]
    fn link_rejects_parent_hash_mismatch_i5() {
        let parent = root_warrant(2_000, 60, 3);
        let mut child = child_warrant();
        child.parent_hash = Some(b"not-the-parent-hash".to_vec());
        let error = verify_link(&parent, &child).expect_err("I5 mismatch");
        assert_eq!(error, AuthorizationError::ParentHashMismatch);
    }

    #[test]
    fn link_rejects_amount_violation_i7() {
        let parent = root_warrant(2_000, 60, 3);
        let mut child = child_warrant();
        child.payment = payment(2_000);
        let error = verify_link(&parent, &child).expect_err("I7");
        assert_eq!(error, AuthorizationError::AmountMonotonicityViolation);
    }

    #[test]
    fn link_rejects_depth_beyond_parent_max() {
        let parent = root_warrant(2_000, 60, 0);
        // Child derived from THIS parent (max_depth = 0): child.depth = 1.
        let child = crate::typestate::DelegatedWarrantBuilder::from(parent.clone()).issue_to(
            delegate_keys().signer_ref(),
            &holder_keys(),
            2_000,
            [0_u8; 8],
        );
        let error = verify_link(&parent, &child).expect_err("I4 static");
        assert!(matches!(error, AuthorizationError::DelegationDepthExceeded { .. }));
    }

    #[test]
    fn link_accepts_valid_child() {
        let parent = root_warrant(2_000, 60, 3);
        let child = child_warrant();
        verify_link(&parent, &child).expect("valid link");
    }

    // ---------------------------------------------------------------------
    // verify_chain proof binding branches
    // ---------------------------------------------------------------------

    #[test]
    fn proof_warrant_id_mismatch_is_rejected() {
        let warrant = root_warrant(2_000, 60, 3);
        let ctx = context(2_000, &holder_keys().signer_ref());
        let mut proof = proof_for(&warrant, &ctx, &holder_keys());
        proof.tuple.warrant_id = vec![0xAB; 16];
        let error = verify_chain(&WarrantChain::single(warrant), &trusted(), &proof, &ctx)
            .expect_err("warrant id mismatch");
        assert_eq!(error, AuthorizationError::WarrantDigestMismatch);
    }

    #[test]
    fn proof_challenge_mismatch_is_rejected() {
        let warrant = root_warrant(2_000, 60, 3);
        let ctx = context(2_000, &holder_keys().signer_ref());
        let proof = proof_for(&warrant, &ctx, &holder_keys());
        let mut other = ctx;
        other.challenge_id = "other-challenge".to_string();
        let error = verify_chain(&WarrantChain::single(warrant), &trusted(), &proof, &other)
            .expect_err("challenge mismatch");
        assert_eq!(error, AuthorizationError::ChallengeMismatch);
    }

    #[test]
    fn proof_request_hash_mismatch_is_rejected() {
        let warrant = root_warrant(2_000, 60, 3);
        let ctx = context(2_000, &holder_keys().signer_ref());
        let proof = proof_for(&warrant, &ctx, &holder_keys());
        let mut other = ctx;
        other.request_hash = sha256_prefixed("other-request");
        let error = verify_chain(&WarrantChain::single(warrant), &trusted(), &proof, &other)
            .expect_err("request hash mismatch");
        assert_eq!(error, AuthorizationError::RequestHashMismatch);
    }

    #[test]
    fn proof_accepted_hash_mismatch_is_rejected() {
        let warrant = root_warrant(2_000, 60, 3);
        let ctx = context(2_000, &holder_keys().signer_ref());
        let proof = proof_for(&warrant, &ctx, &holder_keys());
        let mut other = ctx;
        other.accepted_hash = sha256_prefixed("other-quote");
        let error = verify_chain(&WarrantChain::single(warrant), &trusted(), &proof, &other)
            .expect_err("accepted hash mismatch");
        assert_eq!(error, AuthorizationError::AcceptedHashMismatch);
    }

    #[test]
    fn proof_signer_mismatch_is_rejected() {
        let warrant = root_warrant(2_000, 60, 3);
        let ctx = context(2_000, &holder_keys().signer_ref());
        // Sign proof with an unrelated key => signature does not verify under
        // the leaf holder, which fails before the signer-key equality check.
        let proof = proof_for(&warrant, &ctx, &delegate_keys());
        let error = verify_chain(&WarrantChain::single(warrant), &trusted(), &proof, &ctx)
            .expect_err("signer mismatch");
        assert_eq!(error, AuthorizationError::InvalidProofSignature);
    }

    #[test]
    fn fresh_proof_passes_verification() {
        let warrant = root_warrant(2_000, 60, 3);
        let ctx = context(2_000, &holder_keys().signer_ref());
        let proof = proof_for(&warrant, &ctx, &holder_keys());
        let verified =
            verify_chain(&WarrantChain::single(warrant), &trusted(), &proof, &ctx).expect("valid");
        assert_eq!(verified.chain_len, 1);
        assert_eq!(verified.leaf.id, proof.tuple.warrant_id);
        assert_eq!(verified.root.id, verified.leaf.id);
    }

    #[test]
    fn issued_at_equal_to_current_second_passes() {
        // `node.issued_at > context.now_ms / 1000` is strict: issuing at the
        // exact current second is valid.
        let warrant = root_warrant(2_000, 60, 3);
        // now_ms = 2_000 => now_secs = 2, which equals issued_at = 2.
        let ctx = context(2_000, &holder_keys().signer_ref());
        let proof = proof_for(&warrant, &ctx, &holder_keys());
        let verified = verify_chain(&WarrantChain::single(warrant), &trusted(), &proof, &ctx)
            .expect("issued_at == now is valid");
        assert_eq!(verified.chain_len, 1);
    }

    #[test]
    fn non_leaf_index_check_only_applies_before_last() {
        // A 3-node chain where the middle node has max_depth = 0 must fail
        // (the `index + 1 < len` guard fires for non-leaf nodes), but a
        // single-node chain never enters that branch.
        let root = root_warrant(2_000, 86_400, 0);
        let first = crate::typestate::DelegatedWarrantBuilder::from(root.clone()).issue_to(
            delegate_keys().signer_ref(),
            &holder_keys(),
            2_000,
            [0_u8; 8],
        );
        // The middle (non-leaf) node has max_depth 0 inherited from root.
        let chain = WarrantChain { warrants: vec![root, first] };
        let leaf = chain.leaf().expect("leaf");
        let ctx = context(2_000, &delegate_keys().signer_ref());
        let proof = proof_for(leaf, &ctx, &delegate_keys());
        let error = verify_chain(&chain, &trusted(), &proof, &ctx).expect_err("delegation");
        assert_eq!(error, AuthorizationError::DelegationNotAllowed);
    }

    #[test]
    fn single_node_leaf_with_zero_max_depth_passes() {
        // A single-node chain's only node is a leaf: the delegation-capability
        // guard (`index + 1 < len`) must NOT fire even when max_depth is 0.
        let warrant = root_warrant(2_000, 86_400, 0);
        let ctx = context(2_000, &holder_keys().signer_ref());
        let proof = proof_for(&warrant, &ctx, &holder_keys());
        let verified = verify_chain(&WarrantChain::single(warrant), &trusted(), &proof, &ctx)
            .expect("leaf with max_depth 0 is valid");
        assert_eq!(verified.chain_len, 1);
    }

    // ---------------------------------------------------------------------
    // Multi-node chain boundaries
    // ---------------------------------------------------------------------

    /// Builds a valid chain of `depth + 1` warrants (root at depth 0).
    ///
    /// Each node is held by a distinct deterministic key; the leaf holder is
    /// `keys[depth]`.
    fn build_chain(depth: u32) -> (WarrantChain, Vec<SigningKeyPair>) {
        let mut keys: Vec<SigningKeyPair> = vec![holder_keys()];
        for i in 0..depth {
            keys.push(SigningKeyPair::from_bytes(&[0x60 + i as u8; 32]));
        }
        let mut chain =
            WarrantChain::single(root_warrant(2_000, 86_400, crate::warrant::MAX_DELEGATION_DEPTH));
        for index in 0..depth as usize {
            let parent = chain.warrants.last().expect("parent").clone();
            let next_holder = keys[index + 1].signer_ref();
            let child = crate::typestate::DelegatedWarrantBuilder::from(parent).issue_to(
                next_holder,
                &keys[index],
                2_000,
                [index as u8; 8],
            );
            chain.push(child);
        }
        (chain, keys)
    }

    #[test]
    fn multi_node_chain_verifies_and_reports_depth() {
        let (chain, keys) = build_chain(2);
        assert_eq!(chain.len(), 3);
        let leaf = chain.leaf().expect("leaf");
        let leaf_keys = &keys[2];
        let ctx = context(2_000, &leaf_keys.signer_ref());
        let proof = proof_for(leaf, &ctx, leaf_keys);
        let verified = verify_chain(&chain, &trusted(), &proof, &ctx).expect("3-node chain");
        assert_eq!(verified.chain_len, 3);
        assert_eq!(verified.root.id, chain.warrants[0].id);
    }

    #[test]
    fn non_leaf_node_with_zero_max_depth_rejects_delegation() {
        // Root allows depth 0 => even with a child present, root.max_depth is 0
        // and the child link check fires on the root (index 0).
        let root = root_warrant(2_000, 86_400, 0);
        let child = crate::typestate::DelegatedWarrantBuilder::from(root.clone()).issue_to(
            delegate_keys().signer_ref(),
            &holder_keys(),
            2_000,
            [0_u8; 8],
        );
        let chain = WarrantChain { warrants: vec![root, child] };
        let leaf = chain.leaf().expect("leaf");
        let ctx = context(2_000, &delegate_keys().signer_ref());
        let proof = proof_for(leaf, &ctx, &delegate_keys());
        let error = verify_chain(&chain, &trusted(), &proof, &ctx).expect_err("no delegation");
        assert_eq!(error, AuthorizationError::DelegationNotAllowed);
    }

    #[test]
    fn depth_exceeding_max_is_rejected() {
        // MAX_DELEGATION_DEPTH = 8; a chain of 10 nodes has depth 9 which
        // exceeds the ceiling.
        let (chain, keys) = build_chain(u32::from(crate::warrant::MAX_DELEGATION_DEPTH) + 1);
        let leaf = chain.leaf().expect("leaf");
        let leaf_key = &keys[crate::warrant::MAX_DELEGATION_DEPTH as usize + 1];
        let ctx = context(2_000, &leaf_key.signer_ref());
        let proof = proof_for(leaf, &ctx, leaf_key);
        let error = verify_chain(&chain, &trusted(), &proof, &ctx).expect_err("depth ceiling");
        assert!(matches!(error, AuthorizationError::DelegationDepthExceeded { .. }));
    }

    #[test]
    fn depth_exactly_at_max_passes() {
        let (chain, keys) = build_chain(u32::from(crate::warrant::MAX_DELEGATION_DEPTH));
        let leaf = chain.leaf().expect("leaf");
        let leaf_key = &keys[crate::warrant::MAX_DELEGATION_DEPTH as usize];
        let ctx = context(2_000, &leaf_key.signer_ref());
        let proof = proof_for(leaf, &ctx, leaf_key);
        let verified = verify_chain(&chain, &trusted(), &proof, &ctx).expect("exactly at max");
        assert_eq!(verified.chain_len, crate::warrant::MAX_DELEGATION_DEPTH as usize + 1);
    }

    #[test]
    fn child_expiring_at_exactly_parent_expiry_passes() {
        // The child's expiry is clamped to the parent's remaining lifetime;
        // the I3 check (`child.expires_at > parent.expires_at`) must NOT fire
        // when the child expires no later than the parent.
        let parent = root_warrant(2_000, 86_400, 3);
        let child = crate::typestate::DelegatedWarrantBuilder::from(parent.clone()).issue_to(
            delegate_keys().signer_ref(),
            &holder_keys(),
            2_000,
            [0_u8; 8],
        );
        assert!(child.expires_at <= parent.expires_at);
        verify_link(&parent, &child).expect("child expiry <= parent expiry is allowed");
    }
}
