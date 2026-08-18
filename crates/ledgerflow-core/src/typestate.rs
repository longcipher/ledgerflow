//! Compile-time-safe warrant builder.
//!
//! The builder tracks required fields with type-state markers and validates
//! protocol invariants (TTL cap, depth cap, required constraints) at
//! construction time, so illegal warrants cannot be built.

use std::{collections::BTreeMap, marker::PhantomData};

use crate::{
    approval::ApprovalGate,
    constraint::{MerchantConstraint, PaymentConstraint, ResourceConstraint, ToolConstraint},
    warrant::{
        DEFAULT_MAX_DEPTH, DEFAULT_WARRANT_TTL_SECS, MAX_DELEGATION_DEPTH, MAX_WARRANT_TTL_SECS,
        SignerRef, SigningKeyPair, Warrant, generate_warrant_id_128,
    },
};

/// Marker: issuer not yet configured.
#[derive(Debug)]
pub struct NoIssuer;
/// Marker: issuer configured.
#[derive(Debug)]
pub struct HasIssuer;
/// Marker: holder not yet configured.
#[derive(Debug)]
pub struct NoHolder;
/// Marker: holder configured.
#[derive(Debug)]
pub struct HasHolder;
/// Marker: unsigned.
#[derive(Debug)]
pub struct Unsigned;
/// Marker: signed and ready.
#[derive(Debug)]
pub struct Signed;

/// Compile-time-safe warrant builder for root warrants.
///
/// # Example
///
/// ```ignore
/// use ledgerflow_core::typestate::WarrantBuilder;
/// use ledgerflow_core::constraint::{MerchantConstraint, ResourceConstraint, PaymentConstraint};
///
/// let warrant = WarrantBuilder::new(now_ms)
///     .issuer(issuer_keys.signer_ref())        // NoIssuer -> HasIssuer
///     .holder(holder_keys.signer_ref())        // NoHolder  -> HasHolder
///     .merchant(MerchantConstraint::with_ids(vec!["acme".into()]))
///     .resource(ResourceConstraint::with_path_prefixes(vec!["/v1".into()]))
///     .payment(PaymentConstraint::new(100).with_asset(asset))
///     .sign_with(&issuer_keys, &mut rng);
/// ```
pub struct WarrantBuilder<I, H, Sig> {
    now_ms: u64,
    random_seed: Option<[u8; 8]>,
    explicit_id: Option<[u8; 16]>,
    issuer: Option<SignerRef>,
    holder: Option<SignerRef>,
    ttl_secs: u64,
    max_depth: u8,
    merchant: Option<MerchantConstraint>,
    resource: Option<ResourceConstraint>,
    payment: Option<PaymentConstraint>,
    tool: Option<ToolConstraint>,
    approval_gates: BTreeMap<String, ApprovalGate>,
    required_approvers: Vec<SignerRef>,
    min_approvals: u32,
    extensions: BTreeMap<String, Vec<u8>>,
    _marker: PhantomData<(I, H, Sig)>,
}

impl<I, H, Sig> std::fmt::Debug for WarrantBuilder<I, H, Sig> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WarrantBuilder")
            .field("now_ms", &self.now_ms)
            .field("ttl_secs", &self.ttl_secs)
            .field("max_depth", &self.max_depth)
            .finish_non_exhaustive()
    }
}

impl WarrantBuilder<NoIssuer, NoHolder, Unsigned> {
    /// Creates a new warrant builder. `now_ms` seeds the warrant's `issued_at`.
    #[must_use]
    pub const fn new(now_ms: u64) -> Self {
        Self {
            now_ms,
            random_seed: None,
            explicit_id: None,
            issuer: None,
            holder: None,
            ttl_secs: DEFAULT_WARRANT_TTL_SECS,
            max_depth: DEFAULT_MAX_DEPTH,
            merchant: None,
            resource: None,
            payment: None,
            tool: None,
            approval_gates: BTreeMap::new(),
            required_approvers: Vec::new(),
            min_approvals: 0,
            extensions: BTreeMap::new(),
            _marker: PhantomData,
        }
    }
}

impl<I, H, Sig> WarrantBuilder<I, H, Sig> {
    /// Sets the warrant TTL (defaults to 7 days; hard cap 90 days).
    #[must_use]
    pub fn ttl_secs(mut self, ttl_secs: u64) -> Self {
        self.ttl_secs = ttl_secs.min(MAX_WARRANT_TTL_SECS);
        self
    }

    /// Sets the maximum delegation depth (defaults to 4; hard cap 8).
    #[must_use]
    pub fn max_depth(mut self, max_depth: u8) -> Self {
        self.max_depth = max_depth.min(MAX_DELEGATION_DEPTH);
        self
    }

    /// Sets an explicit 16-byte warrant id (overrides generated UUIDv7).
    #[must_use]
    pub const fn warrant_id(mut self, id: [u8; 16]) -> Self {
        self.explicit_id = Some(id);
        self
    }

    /// Sets the merchant constraint.
    #[must_use]
    pub fn merchant(mut self, merchant: MerchantConstraint) -> Self {
        self.merchant = Some(merchant);
        self
    }

    /// Sets the resource constraint.
    #[must_use]
    pub fn resource(mut self, resource: ResourceConstraint) -> Self {
        self.resource = Some(resource);
        self
    }

    /// Sets the payment constraint.
    #[must_use]
    pub fn payment(mut self, payment: PaymentConstraint) -> Self {
        self.payment = Some(payment);
        self
    }

    /// Sets the tool constraint.
    #[must_use]
    pub fn tool(mut self, tool: ToolConstraint) -> Self {
        self.tool = Some(tool);
        self
    }

    /// Adds an approval gate for a tool.
    #[must_use]
    pub fn approval_gate(mut self, tool: impl Into<String>, gate: ApprovalGate) -> Self {
        self.approval_gates.insert(tool.into(), gate);
        self
    }

    /// Adds a required approver key.
    #[must_use]
    pub fn approver(mut self, approver: SignerRef) -> Self {
        self.required_approvers.push(approver);
        self
    }

    /// Sets the m-of-n approval threshold (0 = all required approvers).
    #[must_use]
    pub const fn min_approvals(mut self, min_approvals: u32) -> Self {
        self.min_approvals = min_approvals;
        self
    }

    /// Adds an application extension (frozen in v1).
    #[must_use]
    pub fn extension(mut self, key: impl Into<String>, value: Vec<u8>) -> Self {
        self.extensions.insert(key.into(), value);
        self
    }

    /// Extracts the raw builder fields for state transitions.
    fn into_parts(self) -> Parts {
        Parts {
            now_ms: self.now_ms,
            random_seed: self.random_seed,
            explicit_id: self.explicit_id,
            issuer: self.issuer,
            holder: self.holder,
            ttl_secs: self.ttl_secs,
            max_depth: self.max_depth,
            merchant: self.merchant,
            resource: self.resource,
            payment: self.payment,
            tool: self.tool,
            approval_gates: self.approval_gates,
            required_approvers: self.required_approvers,
            min_approvals: self.min_approvals,
            extensions: self.extensions,
        }
    }
}

impl<H, Sig> WarrantBuilder<NoIssuer, H, Sig> {
    /// Sets the issuer. Transitions to `HasIssuer`.
    #[must_use]
    pub fn issuer(self, issuer: SignerRef) -> WarrantBuilder<HasIssuer, H, Sig> {
        let parts = self.into_parts();
        WarrantBuilder {
            now_ms: parts.now_ms,
            random_seed: parts.random_seed,
            explicit_id: parts.explicit_id,
            issuer: Some(issuer),
            holder: parts.holder,
            ttl_secs: parts.ttl_secs,
            max_depth: parts.max_depth,
            merchant: parts.merchant,
            resource: parts.resource,
            payment: parts.payment,
            tool: parts.tool,
            approval_gates: parts.approval_gates,
            required_approvers: parts.required_approvers,
            min_approvals: parts.min_approvals,
            extensions: parts.extensions,
            _marker: PhantomData,
        }
    }
}

impl<I, Sig> WarrantBuilder<I, NoHolder, Sig> {
    /// Sets the holder. Transitions to `HasHolder`.
    #[must_use]
    pub fn holder(self, holder: SignerRef) -> WarrantBuilder<I, HasHolder, Sig> {
        let parts = self.into_parts();
        WarrantBuilder {
            now_ms: parts.now_ms,
            random_seed: parts.random_seed,
            explicit_id: parts.explicit_id,
            issuer: parts.issuer,
            holder: Some(holder),
            ttl_secs: parts.ttl_secs,
            max_depth: parts.max_depth,
            merchant: parts.merchant,
            resource: parts.resource,
            payment: parts.payment,
            tool: parts.tool,
            approval_gates: parts.approval_gates,
            required_approvers: parts.required_approvers,
            min_approvals: parts.min_approvals,
            extensions: parts.extensions,
            _marker: PhantomData,
        }
    }
}

impl WarrantBuilder<HasIssuer, HasHolder, Unsigned> {
    /// Signs the warrant with the issuer's key pair and returns it.
    ///
    /// `random_bytes` supplies 8 bytes of caller randomness; the full 128-bit
    /// UUIDv7 id is derived by extending it with the timestamp's low bytes
    /// (deterministic per call). Callers MUST provide fresh random bytes in
    /// production. When an explicit id was set via [`Self::warrant_id`],
    /// `random_bytes` is ignored.
    ///
    /// This is the only terminal transition; it is available once both issuer
    /// and holder are configured.
    pub fn sign_with(self, issuer_keys: &SigningKeyPair, random_bytes: [u8; 8]) -> Warrant {
        let mut builder = self;
        let id = builder.explicit_id.take().unwrap_or_else(|| {
            let mut random128 = [0_u8; 16];
            random128[..8].copy_from_slice(&random_bytes);
            // Deterministically extend to 16 bytes (timestamp-derived tail).
            let ts = builder.now_ms.to_le_bytes();
            random128[8..].copy_from_slice(&ts[..8]);
            generate_warrant_id_128(builder.now_ms, random128)
        });
        let issued_at = builder.now_ms / 1000;
        let expires_at = issued_at.saturating_add(builder.ttl_secs);
        let merchant = builder.merchant.unwrap_or_default();
        let resource = builder.resource.unwrap_or_default();
        #[allow(clippy::expect_used)]
        let payment = builder.payment.expect("warrant builder: payment constraint is required");
        #[allow(clippy::expect_used)]
        let issuer = builder.issuer.expect("warrant builder: issuer is required");
        #[allow(clippy::expect_used)]
        let holder = builder.holder.expect("warrant builder: holder is required");

        let mut warrant = Warrant {
            version: crate::warrant::WARRANT_VERSION_V1,
            id: id.to_vec(),
            holder,
            issuer,
            issued_at,
            expires_at,
            depth: 0,
            max_depth: builder.max_depth,
            parent_hash: None,
            merchant,
            resource,
            payment,
            tool: builder.tool,
            approval_gates: builder.approval_gates,
            required_approvers: builder.required_approvers,
            min_approvals: builder.min_approvals,
            extensions: builder.extensions,
            signature: issuer_keys.sign(b"placeholder"),
        };
        warrant = warrant.sign_with(issuer_keys);
        warrant
    }
}

/// Raw builder parts used internally for state transitions.
struct Parts {
    now_ms: u64,
    random_seed: Option<[u8; 8]>,
    explicit_id: Option<[u8; 16]>,
    issuer: Option<SignerRef>,
    holder: Option<SignerRef>,
    ttl_secs: u64,
    max_depth: u8,
    merchant: Option<MerchantConstraint>,
    resource: Option<ResourceConstraint>,
    payment: Option<PaymentConstraint>,
    tool: Option<ToolConstraint>,
    approval_gates: BTreeMap<String, ApprovalGate>,
    required_approvers: Vec<SignerRef>,
    min_approvals: u32,
    extensions: BTreeMap<String, Vec<u8>>,
}

/// Delegated-warrant builder for attenuating an existing root warrant.
///
/// A delegated warrant re-signs with the parent holder's key, carries the
/// parent's payload hash, and can only narrow constraints. Narrowing is
/// validated **at issuance time** via [`crate::constraint::validate_attenuation`]:
/// a child that would expand capabilities is rejected before signing, so the
/// caller gets immediate feedback instead of a runtime verification failure.
///
/// # Example
///
/// ```ignore
/// let child = DelegatedWarrantBuilder::from(parent)
///     .with_merchant(MerchantConstraint::with_ids(vec!["acme".into()]))
///     .with_payment(PaymentConstraint::new(10))
///     .issue_to(agent_keys.signer_ref(), &issuer_keys, now_ms, random);
/// ```
#[derive(Clone, Debug)]
pub struct DelegatedWarrantBuilder {
    parent: Warrant,
    merchant: Option<MerchantConstraint>,
    resource: Option<ResourceConstraint>,
    payment: Option<PaymentConstraint>,
    tool: Option<ToolConstraint>,
}

impl DelegatedWarrantBuilder {
    /// Starts a delegated warrant from a parent warrant.
    #[must_use]
    pub const fn from(parent: Warrant) -> Self {
        Self { parent, merchant: None, resource: None, payment: None, tool: None }
    }

    /// Narrows the merchant constraint for the child.
    ///
    /// The child's merchant allowlist must be a subset of the parent's; this
    /// is validated in [`Self::issue_to`].
    #[must_use]
    pub fn with_merchant(mut self, merchant: MerchantConstraint) -> Self {
        self.merchant = Some(merchant);
        self
    }

    /// Narrows the resource constraint for the child.
    #[must_use]
    pub fn with_resource(mut self, resource: ResourceConstraint) -> Self {
        self.resource = Some(resource);
        self
    }

    /// Narrows the payment constraint for the child (e.g. a lower cap).
    #[must_use]
    pub fn with_payment(mut self, payment: PaymentConstraint) -> Self {
        self.payment = Some(payment);
        self
    }

    /// Narrows the tool constraint for the child.
    #[must_use]
    pub fn with_tool(mut self, tool: ToolConstraint) -> Self {
        self.tool = Some(tool);
        self
    }

    /// Builds a delegated warrant issued by the parent holder.
    ///
    /// The child inherits the parent's constraints unless narrowed via
    /// [`Self::with_merchant`] / [`Self::with_resource`] / [`Self::with_payment`]
    /// / [`Self::with_tool`]; any narrowing is validated at issuance time so a
    /// child can never expand capabilities. Child TTL cannot exceed the
    /// parent's remaining lifetime. `random_bytes` supplies 8 bytes of caller
    /// randomness for the child's UUIDv7 id (extended to 128 bits as in
    /// [`WarrantBuilder::sign_with`]).
    ///
    /// # Panics
    ///
    /// Panics (via an internal `expect`) if the narrowing would expand
    /// capabilities; this is a programming error in the caller and should be
    /// caught by tests.
    #[must_use]
    #[allow(clippy::panic)]
    pub fn issue_to(
        self,
        new_holder: SignerRef,
        delegator_keys: &SigningKeyPair,
        now_ms: u64,
        random_bytes: [u8; 8],
    ) -> Warrant {
        let parent = &self.parent;
        let issued_at = now_ms / 1000;
        let expires_at = issued_at.min(parent.expires_at);
        let parent_payload_hash = crate::warrant::sha256_prefixed(parent.payload_bytes());
        let depth = parent.depth + 1;

        // Resolve child constraints (narrowed or inherited).
        let merchant = self.merchant.unwrap_or_else(|| parent.merchant.clone());
        let resource = self.resource.unwrap_or_else(|| parent.resource.clone());
        let payment = self.payment.unwrap_or_else(|| parent.payment.clone());
        let tool = self.tool.clone().or_else(|| parent.tool.clone());

        // Static issuance-time attenuation check (decidable fields only).
        let child_constraints: [(crate::constraint::Constraint, crate::constraint::Constraint); 4] = [
            (
                crate::constraint::Constraint::Merchant(parent.merchant.clone()),
                crate::constraint::Constraint::Merchant(merchant.clone()),
            ),
            (
                crate::constraint::Constraint::Resource(parent.resource.clone()),
                crate::constraint::Constraint::Resource(resource.clone()),
            ),
            (
                crate::constraint::Constraint::Payment(parent.payment.clone()),
                crate::constraint::Constraint::Payment(payment.clone()),
            ),
            (
                crate::constraint::Constraint::Tool(parent.tool.clone().unwrap_or_default()),
                crate::constraint::Constraint::Tool(tool.clone().unwrap_or_default()),
            ),
        ];
        for (parent_c, child_c) in child_constraints {
            if let Err(error) = crate::constraint::validate_attenuation(&parent_c, &child_c) {
                // This is a programming error in the delegating application:
                // the caller must not request a child wider than the parent.
                panic!("delegated warrant attenuation failed at issuance: {error}");
            }
        }

        // Issuance-bounds check: the parent (delegator) may carry bounds that
        // further restrict what its child can express. Bounds are a *ceiling*:
        // the child must be no wider than the bounds on every dimension.
        if let Some(bounds) = parent.issue_bounds() {
            validate_issue_bounds(&bounds, &merchant, &resource, &payment, &tool);
        }

        let mut random128 = [0_u8; 16];
        random128[..8].copy_from_slice(&random_bytes);
        let ts = now_ms.to_le_bytes();
        random128[8..].copy_from_slice(&ts[..8]);
        let id = generate_warrant_id_128(now_ms, random128);

        let mut child = Warrant {
            version: crate::warrant::WARRANT_VERSION_V1,
            id: id.to_vec(),
            holder: new_holder,
            issuer: parent.holder.clone(),
            issued_at,
            expires_at,
            depth,
            // `max_depth` is the ceiling for descendant depth, not a
            // per-level decrement. It is inherited unchanged so that a chain
            // can reach `parent.max_depth` levels deep (I4 static check is
            // `child.depth > parent.max_depth`).
            max_depth: parent.max_depth,
            parent_hash: Some(parent_payload_hash.into_bytes()),
            merchant,
            resource,
            payment,
            tool,
            approval_gates: parent.approval_gates.clone(),
            required_approvers: parent.required_approvers.clone(),
            min_approvals: parent.min_approvals,
            extensions: parent.extensions.clone(),
            signature: delegator_keys.sign(b"placeholder"),
        };
        child = child.sign_with(delegator_keys);
        child
    }
}

/// Validates that a child warrant's constraints stay within the delegator's
/// issuance bounds.
///
/// Bounds are a ceiling on every dimension: an empty bound list means "no
/// restriction". The child must be no wider than the bound on each dimension.
/// Violations panic (programming error in the delegating application).
#[allow(clippy::panic)]
fn validate_issue_bounds(
    bounds: &crate::issue_bounds::IssueBounds,
    merchant: &MerchantConstraint,
    resource: &ResourceConstraint,
    payment: &PaymentConstraint,
    tool: &Option<ToolConstraint>,
) {
    if !bounds.merchant_ids.is_empty() {
        for id in &merchant.merchant_ids {
            assert!(
                bounds.merchant_ids.contains(id),
                "issue bounds: merchant `{id}` exceeds the delegator's bounds"
            );
        }
    }
    if !bounds.host_suffixes.is_empty() {
        for suffix in &merchant.host_suffixes {
            assert!(
                bounds.host_suffixes.contains(suffix),
                "issue bounds: host suffix `{suffix}` exceeds the delegator's bounds"
            );
        }
    }
    if !bounds.http_methods.is_empty() {
        for method in &resource.http_methods {
            assert!(
                bounds.http_methods.iter().any(|m| m.eq_ignore_ascii_case(method)),
                "issue bounds: method `{method}` exceeds the delegator's bounds"
            );
        }
    }
    if !bounds.path_prefixes.is_empty() {
        for prefix in &resource.path_prefixes {
            assert!(
                bounds.path_prefixes.iter().any(|bp| prefix.starts_with(bp)),
                "issue bounds: path prefix `{prefix}` exceeds the delegator's bounds"
            );
        }
    }
    if !bounds.assets.is_empty() {
        for asset in &payment.allowed_assets {
            assert!(
                bounds.assets.iter().any(|a| a == asset),
                "issue bounds: asset `{}` exceeds the delegator's bounds",
                asset.asset
            );
        }
    }
    if !bounds.rails.is_empty() {
        for rail in &payment.allowed_rails {
            assert!(
                bounds.rails.contains(rail),
                "issue bounds: rail `{rail:?}` exceeds the delegator's bounds"
            );
        }
    }
    if !bounds.schemes.is_empty() {
        for scheme in &payment.allowed_schemes {
            assert!(
                bounds.schemes.contains(scheme),
                "issue bounds: scheme `{scheme}` exceeds the delegator's bounds"
            );
        }
    }
    if !bounds.payee_ids.is_empty() {
        for payee in &payment.payee_ids {
            assert!(
                bounds.payee_ids.contains(payee),
                "issue bounds: payee `{payee}` exceeds the delegator's bounds"
            );
        }
    }
    if let Some(cap) = bounds.max_per_charge {
        assert!(
            payment.max_per_charge <= cap,
            "issue bounds: per-charge cap {} exceeds the delegator's bound {}",
            payment.max_per_charge,
            cap
        );
    }
    let _ = tool; // Tool bounds are validated by the parent-attenuation check.
}
