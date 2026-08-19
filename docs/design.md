# LedgerFlow Generic Transaction Component — Design Document

> Status: Draft (v0.2, revised per technical review)
> Scope: Overall design for the LedgerFlow major refactor / rewrite

---

## 1. Executive Summary

LedgerFlow is a **self-hostable / SaaS-deployable, protocol-agnostic generic
transaction component**. It turns the missing **Authz (authorization) layer**
of the existing x402 / MPP payment protocols into an independent, portable
protocol extension, and uses a lightweight **Facilitator** to route verified
payments to arbitrary settlement rails (EVM / Solana / Tempo / Stripe /
traditional gateways).

The core of this refactor is not "rewriting code" but **redefining
boundaries**:

1. **Authz layer becomes an independent protocol**: Warrant (capability
   token) → delegation chain → PoP → approval gates → revocation;
2. **Pluggable protocol bindings**: the same Authz layer binds as extensions
   to x402 (v2 extensions mechanism) and to MPP (Payment HTTP auth scheme
   extension);
3. **Pluggable settlement routing**: the Facilitator decouples from
   specific chains and connects to rails through standard interfaces;
4. **Standard-protocol wallet integration**: defines the `WalletSigner`
   capability interface; no dependency on any specific wallet;
5. **Dual deployment modes**: standalone (single-user self-hosted) and saas
   (multi-tenant gateway integration) share the same codebase.

> Design red line: **no coupling to OneCipher and no coupling to LongCipher
> Platform**. LedgerFlow is an open-source protocol-layer project; any wallet
> and any SaaS platform can integrate through standard protocols / standard
> APIs. OneCipher / LongCipher Platform are its first two integration cases,
> not dependencies.

### 1.1 Verifier Model (v0.2 revision core — read this first)

v0.1 claimed "all authorization decision data lives in the token, no online
authorization dependency", while also promising periodic limits, cumulative
budgets, and revocation — a contradiction. v0.2 adopts an explicit **hybrid
Verifier Model** that draws a clear boundary between "offline verifiable" and
"stateful checks":

| Check | Nature | Executed at | Offline? |
|---|---|---|---|
| Signature-chain verification (cryptographic parts of I1–I7) | stateless | any verifier (merchant / Facilitator / agent self-check) | ✅ offline |
| PoP proof-of-possession | stateless | any verifier | ✅ offline |
| Stateless constraints (per-charge cap, merchant, resource, TTL) | stateless | any verifier | ✅ offline |
| **Revocation check** | stateful | **the single accounting Facilitator / merchant online against RevocationStore** | ❌ online |
| **Periodic limits / cumulative budget** | stateful | **the single accounting Facilitator** | ❌ online |

Corollaries (written into the spec semantics):

- **The v1 constraint set contains only stateless predicates** (per-charge
  cap, merchant, resource, TTL); periodic limits / cumulative budgets are
  removed from the v1 constraint set and enter the roadmap as an
  "accounting-point mode" (P2+, §14);
- **Budget enforcement for a warrant binds to exactly one accounting
  Facilitator** (the warrant declares `ledgerflow.ledger`); an agent cannot
  overdraw across accounting points;
- **Revocation is an online security commitment**, not part of "offline
  verification"; the Facilitator persists revocation records (§6.6).

---

## 2. Background and Problem Statement

### 2.1 Protocol Landscape

| Protocol | Status | Problem solved | Missing |
|---|---|---|---|
| **x402** (Coinbase / x402-foundation) | v2 spec, RFC-ward | HTTP 402 semantics: `PaymentRequired` → `PaymentPayload` → Facilitator verification and settlement; `exact` / `upto` / `batch-settlement` schemes; CAIP-2 network identifiers; **native extensions mechanism** (server advertises, client echoes) | **No authorization semantics**: holding the wallet private key is sufficient to pay; cannot express "this payment is authorized, amount-capped, approval-gated, revocable" |
| **MPP** (Tempo + Stripe, paymentauth.org) | IETF draft (`draft-ietf-httpauth-payment`) | `WWW-Authenticate: Payment` challenge + `Authorization: Payment` response HTTP auth scheme; charge / session intents; multiple chains (Tempo / Solana / Hedera / Lightning / Stripe) | Same **lack of task-level authorization**; sessions (streaming payments) especially need authorization boundaries |

**The common gap**: in the AI-agent economy, the paying party is often an
"agent executing a delegated task", not the resource owner. Existing protocols
assume "holding the private key = authorized to pay", which leads to:

- **Uncontrolled spending**: a single prompt injection can drain the wallet;
- **No delegation boundaries**: cannot express "only for merchant X,
  resource Y, per-charge at most Z";
- **No human approval**: high-value payments cannot require m-of-n human
  confirmation;
- **No revocation**: no way to revoke an authorization before settlement once
  anomalies are found;
- **No audit trail**: cannot answer "who authorized whom, with what
  constraints, whether the chain is legitimate".

### 2.2 The LedgerFlow Answer

> **LedgerFlow is not yet another payment protocol; it is an authorization
> layer on top of payment protocols.**
> It keeps x402 / MPP as the merchant↔agent wire protocol unchanged, carries
> LedgerFlow authorization data through **protocol extensions**, and routes
> verified payments to settlement rails through a small Facilitator.

The existing codebase has already taken the first steps
(`ledgerflow-core`'s Warrant / Proof / 5 constraint types / typestate
verification pipeline, `ledgerflow-x402`'s challenge and payload extensions),
but is missing: delegation chains, approval gates, revocation, trust model,
MPP binding, wallet integration abstraction, SaaS mode, and deployable
services.

---

## 3. Research and Benchmarking

### 3.1 Authz-Layer Design Principles

The Authz-layer design builds on well-established capability-token patterns
(attenuating capability systems and signed-delegation chains) and adapts them
to payment semantics under the LedgerFlow naming.

| Established pattern | LedgerFlow adaptation |
|---|---|
| Warrant: signed envelope (CBOR payload + Ed25519 signature), UUIDv7 id | adopted; fields redefined for payment semantics (§6.1) |
| Delegation chain: `parent_hash` cryptographic link + `depth/max_depth`, six invariants I1–I6 | adopted and adapted; **attenuation now uses runtime conjunction** (§6.2), avoiding the undecidable static-subset problem |
| Two warrant types (Execution / Issuer) | collapsed into a single **Payment Warrant** (Issuer semantics expressed naturally by the delegation chain) |
| Constraint lattice (partial order) + monotonic attenuation + fail-closed | **partially adopted**: fail-closed kept; static subset judgment only for decidable fields (exact id, numeric, TTL, depth); the rest uses runtime conjunction |
| PoP (~2 min window, bound to request tuple) | adopted; binding tuple changed to CBOR-deterministic structured data (§6.3) |
| Approval gates + m-of-n multisig (`required_approvers` + `min_approvals` + SignedApproval) | adopted for "high-value payments need human approval", with notification channel (§6.5) |
| SRL (Signed Revocation List) + heartbeat sync | **downgraded to roadmap**: v1 uses a persisted RevocationStore with online checks (§6.6) |
| Control/Data plane separation (feature gated) | adopted as deployment modes rather than crate splits (§5.3) |
| IETF AAT draft alignment | conceptual alignment only; no spec-chasing |

**Deliberately not done** (other capability systems have it, LedgerFlow does
not need it): 16+ constraint types, CEL expressions, language-binding matrix
(Python/WASM), orchestrator, gateway templates. LedgerFlow converges to the
minimal complete set for payment semantics.

### 3.2 Protocol Implementation References

| Project | Takeaway |
|---|---|
| **r402** (qntx) | Facilitator trait / hooks lifecycle, pluggable `SchemeRegistry`, layered HTTP transport (paygate/server/client separation) |
| **x402-rs** (x402-foundation) | v1/v2 type layering (Types / Logic / Representation), cross-implementation conformance tests (TS↔Rust), `docs/specs/` spec-as-code |
| **mpp-rs** (Tempo) | four-layer architecture (Application/Methods/Intents/Core), `PaymentProvider` trait, session state machine, multi-transport (HTTP/WS/MCP) |

### 3.3 Ecosystem and Peer Projects (web research)

| Project | Positioning | Insight for LedgerFlow |
|---|---|---|
| **PayAI / second-state x402-facilitator** | production-grade x402 Facilitator (Solana-first, OpenTelemetry observability) | Facilitator is an independently deployable component; Otel observability is baseline |
| **Stripe Machine Payments / MPP** | unified card + stablecoin acceptance | header-based auth scheme and x402 payload-based scheme coexist; that is the reality |
| **Coinbase x402 whitepaper (2026-07)** | agentic-commerce payment standard | ecosystem evolves toward "standard + extension + facilitator"; LedgerFlow should be a "standard on top of the standard" |
| **Payman / Centuria / Orbit / Spark** | commercial AI payment layers | confirm that "authorization/budget/approval" is a hard requirement; all closed implementations; LedgerFlow differentiates as an open protocol layer |
| **WalletConnect v2 / CAIP family** | wallet interop standards | wallet interaction must go through standard protocols (WC v2 + CAIP-2/10/19) |

### 3.4 Key Insights (design inputs)

1. **Extensions are the right mechanism**: x402 v2 native extensions (server
   advertises info+schema, client echoes) and MPP challenge parameters both
   allow lossless LedgerFlow superposition without forking protocols;
2. **Authorization verification can be offline; budget and revocation must be
   online**: signature chains + PoP + stateless constraints verify offline
   (the scalability premise); budget and revocation are state predicates
   handled by the accounting Facilitator (§1.1 Verifier Model);
3. **Wallets must integrate through standard protocols**: LedgerFlow depends
   on the "wallet capability interface" (`WalletSigner` trait), not any
   specific wallet;
4. **Dual-mode deployment is one codebase, two modes**: standalone/saas
   differ only in identity source and tenant context; business logic shared;
5. **No IAM in the protocol layer**: identity/user systems belong to host
   applications; LedgerFlow only cares about "keys + authorization +
   settlement".

---

## 4. Design Goals, Principles, and Scope

### 4.1 Design Goals (priority order)

1. **Generic**: one protocol layer serving both x402 and MPP; no binding to
   any wallet, platform, or chain;
2. **Deployable**: standalone runs with one command (single-user self-host);
   saas mode integrates into multi-tenant platforms through a standard
   gateway;
3. **Secure**: delegation chains, PoP, approvals, revocation, and trust
   anchors complete; fail-closed;
4. **Portable**: core is I/O-free and async-free; embeddable in any host
   (Rust services, WASM, future SDKs);
5. **No over-engineering**: MVP only does the minimal complete set for payment
   authorization; the rest goes to the roadmap.

### 4.2 Design Principles

- **Protocol-layer neutrality**: LedgerFlow extensions must be expressed
  within protocol mechanisms (x402 extensions / MPP challenge parameters),
  never forked;
- **Standard-interface integration**: wallets via the standard signing
  interface + local JSON-RPC; SaaS via the gateway internal-header protocol;
  rails via CAIP-2/19 standardized identifiers;
- **Honest verification model**: authorization verification (signature chains
  - PoP + stateless constraints) is offline; revocation and budget are online
  (§1.1); never overstate the "offline" promise;
- **fail-closed**: unknown constraints, unknown extensions, and unknown
  warrant extensions are all rejected; invalid configuration = startup
  failure (fail-fast), never a silent downgrade;
- **Type safety**: preserve the typestate verification pipeline philosophy so
  illegal states cannot be constructed;
- **Rust engineering standards**: follow AGENTS.md (hpx > reqwest, scc >
  dashmap, eyre/thiserror layering, tracing + OTel observability).

### 4.3 Scope Boundaries (out of scope, deliberately)

| Not done | Reason |
|---|---|
| User identity / login / SSO | belongs to the host platform (SaaS gateway provides it) |
| Task orchestration / agent frameworks | application layer |
| Proprietary settlement chains / stablecoins | routing only; no invented rails |
| Language-binding matrix (full Python/TS SDKs) | on demand later; WASM first proves portability |
| IAM / fine-grained RBAC | the constraint system already covers the payment granularity; the `x-internal-roles` header is only relayed for host audit, not consumed by LedgerFlow |
| Client-side budget management / session mechanisms | x402 spec explicitly out of scope; LedgerFlow budget expressed via the accounting point (P2+) |
| Gasless / sponsored payments (paymaster) | deferred together with `SponsorshipConstraint` (§6.4, §14) |

---

## 5. Overall Architecture

### 5.1 Layered Architecture

```text
┌────────────────────────────────────────────────────────────────┐
│ Application Layer (host applications, not owned by LedgerFlow) │
│   Merchant API · Agent Runtime · SaaS Platform · Wallet UI      │
├────────────────────────────────────────────────────────────────┤
│ Protocol Wire Layer (wire protocols, standards untouched)       │
│   x402: 402/PaymentRequired → PaymentPayload (HTTP/MCP/A2A)     │
│   MPP:  WWW-Authenticate: Payment → Authorization: Payment      │
│            ▲ extensions / challenge-params carry LedgerFlow      │
├────────────────────────────────────────────────────────────────┤
│ LedgerFlow Authz Layer (project core, transport-agnostic)       │
│   Warrant · delegation chain · PoP · stateless constraints ·    │
│   approval gates · trust anchors                                │
│   ── ledgerflow-core (pure domain, no I/O, stateless)           │
│   ── ledgerflow-protocol (x402/MPP/HTTP binding codecs)         │
├────────────────────────────────────────────────────────────────┤
│ LedgerFlow Facilitator (settlement routing + stateful checks)   │
│   /verify (stateless verification + revocation pre-check)       │
│   /settle (atomic re-verification + settlement)                 │
│   RevocationStore (online revocation) · budget accounting (P2+) │
│   RailAdapter: EVM · Solana · Tempo · Stripe · traditional gw   │
├────────────────────────────────────────────────────────────────┤
│ LedgerFlow Server (deployment modes, optional loading)          │
│   standalone (single-user) · saas (gateway + tenant context)    │
│   REST API · Webhook · Admin (warrant issuance / revocation /   │
│   audit)                                                        │
└────────────────────────────────────────────────────────────────┘
```

### 5.2 Component Responsibility Matrix

| Component | Responsibility | Not responsible for |
|---|---|---|
| `ledgerflow-core` | authorization domain types, **stateless** verification logic, invariants, `RevocationCheck` trait definition | no I/O / network / storage / online state |
| `ledgerflow-protocol` | x402 / MPP extension codecs, HTTP carriers, middleware | no settlement logic |
| `ledgerflow-facilitator` | payment-verification orchestration + rail routing + **online revocation checks** | no JWT parsing, no user identity |
| `ledgerflow-server` | REST API, webhook, admin, SaaS mode, revocation persistence | no authorization semantics (delegated to core) |
| `ledgerflow-cli` | fixture generation, issuance/revocation tooling, demos | — |

### 5.3 Deployment Modes (dual-mode, one codebase)

```text
                    standalone (default)                     saas
Identity source   none (local keys / configured anchors)     gateway-injected x-internal-* headers + service token
Tenant context    fixed default                              isolated by x-internal-tenant-id
Listener          local / single-host                         gateway origin (mTLS / network policy enforced)
Wallet            local JSON-RPC / in-process signer         WC v2 (cross-process/remote) + local signer
Revocation        persisted (SQLite/file); in-memory demo     persisted (same DB as tenant data)
                  only with explicit --insecure-revoc-memory
```

**Configuration semantics (fail-fast, v0.2 revision)**:

- A completely absent `[saas]` section → **explicit default** to standalone
  (backward-compatible migration path, not an error fallback);
- A present `[saas]` section with an invalid `mode` / missing fields / missing
  service token → **startup failure** (fail-fast); the error message names the
  missing field;
- Startup self-checks: trusted issuers non-empty, revocation store reachable,
  service token configured in saas mode; exit on failure;
- **No "configuration error → silent downgrade" path is allowed**: this is a
  security commitment (fail-closed); availability must never be traded for
  security.

---

## 6. Core Protocol Design: LedgerFlow Authz Layer

### 6.1 Warrant Model (v1, CBOR)

Signed envelope (keeps the existing implementation, minor field tweaks):

```text
SignedWarrant {
    envelope_version: u8,          // = 1
    payload: bytes,                // CBOR(WarrantPayload)
    signature: Signature,          // Ed25519 over "ledgerflow-warrant-v1" || envelope_version || payload
}
```

Payload fields (payment-semantic; v0.2 revisions: tighter depth/TTL caps,
sponsorship deferred, extensions frozen):

| Field | Type | Required | Description |
|---|---|---|---|
| `version` | u8 | ✓ | payload version = 1 |
| `id` | bytes[16] | ✓ | UUIDv7 |
| `holder` | SignerRef | ✓ | authorized user's public key (agent) |
| `issuer` | SignerRef | ✓ | issuer's public key (human / control plane) |
| `issued_at` / `expires_at` | u64 | ✓ | Unix seconds; **default TTL 24h–7d, hard cap 90 days** |
| `depth` / `max_depth` | u8 | ✓ | **default 4, hard cap 8 (configurable)** |
| `parent_hash` | bytes[32] | ✗ | see §6.2 I5 (domain-separated hash); null at root |
| `merchant` | MerchantConstraint | ✓ | allowed merchants (exact id / domain prefix / pubkey) |
| `resource` | ResourceConstraint | ✓ | allowed resources (URL prefix pattern / path) |
| `payment` | PaymentConstraint | ✓ | **stateless per-charge cap**: (asset: CAIP-19, max_per_charge: base units) |
| `tool` | ToolConstraint | ✗ | optional: tool-call whitelist (agent scenarios) |
| `approval_gates` | map<tool, ConstraintSet> | ✗ | call patterns that trigger approval |
| `required_approvers` | array<SignerRef> | ✗ | approver public keys |
| `min_approvals` | u32 | ✗ | m-of-n threshold |
| `extensions` | map | ✗ | **frozen in v1: unknown keys rejected (fail-closed)** |

> v0.2 revisions: `SponsorshipConstraint` is deferred together with paymaster;
> v1 schema reserves no placeholder. `PaymentConstraint` narrows from
> "per-charge + periodic + cumulative" to a **stateless per-charge cap**
> (periodic/cumulative enter accounting-point mode, §1.1, §14). The existing
> `AssetRef` / `PaymentRail` / `AmountLimit` types are kept; `PeriodLimit`
> leaves the v1 constraint set.

### 6.2 Delegation and Attenuation (protocol invariants)

Defines invariants I1–I6 adapted for payment semantics; **attenuation uses
runtime conjunction** (v0.2 revision):

| ID | Invariant |
|---|---|
| I1 | `child.issuer == parent.holder` (delegation authority transfer) |
| I2 | `child.depth == parent.depth + 1` |
| I3 | `child.expires_at ≤ parent.expires_at` (TTL monotonicity) |
| I4 | **capability monotonicity (runtime conjunction)**: at verification time, the request must satisfy the merchant/resource/tool constraints of every node in the chain |
| I5 | `child.parent_hash == SHA256("ledgerflow-warrant-v1" ‖ envelope_version ‖ parent.payload)` (domain-separated) |
| I6 | PoP signature verifies under the `holder` public key |
| **I7** | **amount monotonicity (numeric comparison)**: child's per-charge cap ≤ parent's per-charge cap |

**Attenuation algorithm (spec-level)**:

- **Runtime conjunction** (I4): the verifier runs `verify(node_i, req)` per node;
  all must pass. Attenuation is guaranteed naturally by per-node checks — the
  child need not be a static subset of the parent, as long as the request
  passes every node's constraints. **No URL-pattern language-containment
  judgment** (undecidable in general).
- **Decidable static fields** (I3, I7, depth): exact numeric comparisons,
  judged directly.
- fail-closed: unknown constraint types, unknown constraint fields, and
  unknown warrant extensions are all rejected.

> Design trade-off note: a full constraint lattice (partial order +
> monotonic attenuation judgment) depends on a decidable subset of a custom
> DSL and is complex to implement. LedgerFlow trades static containment for
> decidability and interop via runtime conjunction; the cost is the loss of
> immediate "static rejection of over-limit child at issuance". Since every
> payment still verifies node-by-node, the security semantics are equivalent
> (every node's constraints must be satisfied). Issuers may still run static
> pre-checks on decidable fields (merchant exact id, amount, TTL); undecidable
> fields await runtime verification.

### 6.3 PoP and Replay Protection

- On every payment call, the agent signs a **structured binding tuple** with
  the holder private key (v0.2 revision: dropped `canonical_request` text
  canonicalization; CBOR deterministic encoding, RFC 8949 Core Deterministic
  Encoding):

```text
b"ledgerflow-pop-v1" ‖ CBOR{
    warrant_id, challenge_id, method, uri, payment_payload_digest,
    approvals_digest,   // required when approvals exist (closes the §2.6 concat ambiguity)
    nonce, timestamp
}
```

- **Clock tolerance**: verifiers accept `|now - timestamp| ≤ 60s + skew
  (default 30s)`; skew is configurable;
- **One-time challenge**: MerchantVerifier enforces challenge validity (default
  5 min) plus use-once burn (single-payment binding);
- Combined with x402's `payment_identifier` / MPP's challenge nonce for
  idempotency;
- The existing `ReplayStore` is kept (`challenge_id + nonce` fingerprint +
  payment-identifier idempotency); loss of in-memory state across restart is
  acceptable (combined with payment_identifier idempotency; documented as a
  known limitation);
- **Strict Ed25519 verification**: canonical signatures enforced (`s < l`);
  non-canonical signatures rejected; included in interop test vectors.

### 6.4 Constraint System (converged set)

After v0.2, the v1 constraint set contains only **stateless predicates**:

| Constraint | Example semantics | Judgment |
|---|---|---|
| `MerchantConstraint` | `merchant_id == "acme"`, `domain prefix "*.acme.com"` | prefix wildcard (decidable) |
| `ResourceConstraint` | `https://api.acme.com/v1/*` (URL prefix) | prefix wildcard (decidable) |
| `PaymentConstraint` | `(asset: eip155:8453/slip44:60, max_per_charge: 100_000_000)` (USDC base units) | numeric comparison (decidable) |
| `ToolConstraint` | `search` / `read` call whitelist | exact match (decidable) |

**Amount semantics (spec-level)**:

- `PaymentConstraint` MUST be `(asset: CAIP-19, max_per_charge: u128)`; amounts
  are always **base units** (smallest on-chain unit), eliminating
  "100 vs 100_000_000" unit confusion;
- Missing or invalid asset = reject (fail-closed);
- Settlement rail is not restricted by the merchant constraint; rail selection
  is Facilitator routing responsibility (§8).

**Deferred (not in the v1 schema)**: `PeriodLimit` (periodic limits),
`SponsorshipConstraint` (sponsorship/paylater), CEL/Regex long-tail
constraints — introduced together with accounting-point mode (P2+) and
paymaster (roadmap).

### 6.5 Approval Gates (m-of-n human approval)

For high-value / high-risk payments (m-of-n approval pattern):

```text
Agent initiates payment (no approval)
  → merchant/Facilitator hits an approval_gate while verifying the warrant
  → returns approval_required (with request_hash = binding-tuple hash)
  → approver (human / wallet holder) signs SignedApproval over request_hash
  → approval notification: webhook event + wallet push (approval.requested),
    with backoff-retry semantics
  → agent retries with approvals (array)
  → threshold min_approvals reached → verification and settlement proceed
```

- `SignedApproval`: `Ed25519 over "ledgerflow-approval-v1" || request_hash ||
  approver_pubkey || exp`;
- Approval TTL default 300 s; non-delegatable (only keys in
  `required_approvers` are valid);
- **Approver key rotation**: documented as a v1 limitation (fixed key set at
  issuance); key rotation on the roadmap;
- The PoP tuple includes `approvals_digest` (§6.3), closing the approvals/PoP
  concatenation ambiguity.

### 6.6 Revocation

- **Production requires persistence**: the Facilitator / Server holds a
  `RevocationStore` (SQLite / file) recording `warrant_id / holder / subject`
  revocations; **still effective after restart**;
- **standalone demos**: an in-memory `RevocationStore` is allowed but must be
  explicitly declared with `--insecure-revoc-memory` (startup banner
  warning);
- **Check timing**: `/verify` pre-check + `/settle` atomic re-check
  (§8.1 closes the TOCTOU);
- **MPP session semantics**: revocation takes effect at the **next session
  tick**, with the Facilitator actively closing the stream; the acceptance
  window (≤ 1 tick) is documented;
- **Roadmap enhancement**: Signed Revocation List (SRL) + heartbeat sync, so
  revocation propagates across nodes and is auditable.

### 6.7 Signatures and Domain Separation

Uses domain-separated signing to prevent cross-purpose replay:

| Domain prefix | Purpose |
|---|---|
| `ledgerflow-warrant-v1` | warrant envelope signature (parent_hash same domain, see I5) |
| `ledgerflow-pop-v1` | proof-of-possession |
| `ledgerflow-approval-v1` | approval signature |
| `ledgerflow-srl-v1` | revocation-list signature (roadmap) |

Algorithm: Ed25519 is the only mandatory v1 algorithm (strict canonical
verification, §6.3); the `SigningAlgorithm::Secp256k1` enum variant is
reserved for EVM-side wallet direct signing (roadmap).

### 6.8 Trust Model (new in v0.2)

- **Trust anchors**: each merchant / Facilitator configures a **trusted
  issuers set** (`key_id + public_key`); the chain's **root issuer MUST match
  this set**, otherwise reject (fail-closed);
- **Trusted issuers are merchant-side identity configuration**: unrelated to
  the challenge's `merchant_id` (the merchant self-asserted identity); the two
  must not be confused;
- **Multi-tenant**: in saas mode, issuance keys and trust anchors are isolated
  per `tenant_id` (§10.2); tenant A's anchors do not affect tenant B;
- **Key rotation**: dual-signature window (old and new roots both valid for N
  days; warrants issued by the new root carry the key id); the old root leaves
  the set after expiry;
- **Hot configuration**: `ArcSwap` carries the trusted-issuers config for
  hot updates (per AGENTS.md).

---

## 7. Transport Bindings (protocol extensions)

### 7.1 x402 Binding (v2 extensions mechanism)

Uses x402 v2 **native extensions** (server advertises `info` + `schema`,
client echoes):

```text
In the 402 PaymentRequired extensions:
  "ledgerflow": {
    "info": {
      "version": "1",
      "challenge_id": "<uuid>",
      "merchant_id": "acme",
      "resource": "https://api.acme.com/v1/*",
      "required_subject_kinds": ["payment"],
      "approval_policy": "none" | "m-of-n",
      "warrant_required": true,
      "ledger": "https://ledger.example"      // accounting point (P2+ budget; null in v1)
    },
    "schema": { ... JSON Schema of info ... }
  }

In the PaymentPayload extensions (client echo):
  "ledgerflow": {
    "version": "1",
    "challenge_id": "<echo>",
    "warrant_chain": [ <CBOR warrant> × N ],   // full delegation chain, inline (§7.1)
    "proof": { "alg": "ed25519", "value": "..." },
    "signer": { "alg": "ed25519", "public_key": "..." },
    "payment_subject": { "kind": "payment", "ref": "..." },
    "approvals": [<base64(CBOR SignedApproval)>]
  }
```

**Chain transport semantics (v0.2 revision, closes the witness problem)**:

- v1 requires the **warrant chain to be transmitted inline in full**
  (`warrant_chain` array, root first, leaf last);
- **Digest references are only for already-cached warrants**: the cache
  establishment protocol is "first inline transmission + `cacheable: true`
  declared in the challenge"; merchants cache by `warrant.id` and may later
  send digest references;
- **No undefined remote-resolution protocol**: v1 does not implement a
  "fetch warrant by digest from issuer" channel.

The merchant-side `MerchantVerifier` middleware is kept, extended with
approval-gate triggering, revocation pre-checks, one-time challenges, and
trusted-issuer validation.

### 7.2 MPP Binding (Payment HTTP auth scheme extension)

MPP is based on `draft-ietf-httpauth-payment`:
`WWW-Authenticate: Payment challenge-params` →
`Authorization: Payment response`. LedgerFlow superposes via **challenge
parameters + auth-scheme parameters**, without invading charge/session
semantics:

```text
WWW-Authenticate: Payment
  method="charge",
  params="...",                            // original MPP params
  ledgerflow="<base64url(CBOR LedgerFlowChallenge)>"   // extension param

Authorization: Payment
  method="charge",
  params="...",
  ledgerflow="<base64url(CBOR LedgerFlowAuthorizationExtension)>"
```

**Header size constraint (v0.2 revision, closes the size wall)**:

- HTTP-header mode allows **only a single-layer warrant / digest reference**
  (single CBOR node + base64url ≤ ~2 KB);
- **The full chain goes in the body**: in charge scenarios, the complete chain
  is carried in the 402 response body (or a dedicated
  `/ledgerflow/chain` endpoint); the header carries only the root digest;
- In multi-challenge / multi-scheme scenarios, chain caching reuses the
  "first inline + digest reference" mechanism (§7.1).

Implemented as a wrapper in the mpp-rs-style trait layer:
`LedgerFlowChargeMethod` wraps a concrete `ChargeMethod`, verifying authz
before payment; supports MPP's HTTP / WS / MCP transports (header params /
handshake params / `_meta`); in session scenarios the warrant binds to the
session lifetime; revocation semantics per §6.6.

### 7.3 Generic Carriers (non-HTTP scenarios)

| Carrier | Carried as | Size policy |
|---|---|---|
| MCP | `params._meta.ledgerflow` | chain inline (no header limit) |
| A2A | header / params | same as §7.2 (header single node, chain in message body) |
| In-process | direct core verification API call | unlimited |

---

## 8. Facilitator Design

### 8.1 Responsibilities and API

```text
POST /verify    input: PaymentPayload(x402) or Payment response(MPP) + ticket context
                → stateless authz verification (warrant chain + PoP + stateless
                  constraints + approvals + trust anchors)
                → revocation pre-check (RevocationStore)
                → payment-credential validation (scheme-specific)
                → return verified + routing suggestion (verify is a pre-check,
                  not final authorization)

POST /settle    input: verified session + rail selection
                → 【atomic re-verification】revocation + TTL + PoP freshness +
                  amount cap (closes the TOCTOU)
                → submit settlement, return receipt (tx_hash / settlement proof)
                → the settlement action and the final revocation check happen
                  in the same atomic operation

GET  /status    idempotent settlement-status query
```

The existing `Facilitator / RailKind / RouteDecision / RailAdapter` trait
system is kept; adds `VerifyOutcome` structured error codes (distinguishing
`unauthorized` / `insufficient_approval` / `replayed` / `expired` / `revoked`
/ `invalid_payment`, aligned with x402's ErrorReason semantics).

### 8.2 Rail Adapters (RailAdapter)

| Rail | Status | Notes |
|---|---|---|
| EVM | ✓ (demo adapter) | `exact` / `upto` / `batch-settlement` schemes |
| Solana | ✓ (skeleton adapter) | SPL Token / Token-2022 exact |
| Exchange | ✓ (demo adapter) | off-chain exchange settlement |
| Custodial | ✓ (demo adapter) | custodial ledger settlement |
| Gateway | ✓ (demo adapter) | traditional payment-gateway settlement |
| Tempo | roadmap | MPP charge/session (reusing the mpp-rs approach) |
| Stripe | roadmap | card acquiring (SPT) |

> All rail adapters are **demo-grade** in v1: they return deterministic receipts
> so the orchestration and TOCTOU-closing logic can be exercised end-to-end.
> Real chain integrations (EVM RPC, Solana, Tempo, Stripe) replace the adapter
> internals without changing the `RailAdapter` trait.

The Facilitator stays **rail-agnostic at the merchant boundary** (existing
principle), exposing only `verify/settle/status`; rail selection is routing
responsibility, not restricted by warrant constraints.

### 8.3 Observability

tracing + OpenTelemetry OTLP (traces/metrics), per AGENTS.md; the Facilitator
is an independently deployable binary (optional), modeled on
second-state x402-facilitator; revocation / approval / settlement events are
written to the audit record (append-only, §13.7).

---

## 9. Wallet Integration Design

### 9.1 Integration Principles

> **LedgerFlow does not depend on any wallet implementation**, only on the
> "wallet capability interface" (`WalletSigner` trait). Any wallet satisfying
> the interface (including OneCipher, remote WC v2 wallets, and custom
> signers) can integrate.

Defines the `WalletSigner` abstraction (in the `ledgerflow-wallet` crate):

```rust
pub trait WalletSigner: Send + Sync {
    /// Wallet identifier (URI / name).
    fn descriptor(&self) -> WalletDescriptor;

    /// Lists the keys available in this wallet.
    fn keys(&self) -> Result<Vec<SignerRef>, WalletError>;

    /// Signs an arbitrary message.
    fn sign(&self, request: &SignRequest) -> Result<SignResult, WalletError>;

    /// Signs on-chain payment transactions (called by rail schemes).
    fn sign_payment(&self, request: &SignPaymentRequest) -> Result<SignedPayment, WalletError>;
}
```

> The interface is **synchronous** by design: the underlying operations are
> local signing or short-lived transport calls, which keeps embedded /
> in-process signers simple and avoids blocking an async runtime. HTTP-backed
> signers perform their transport call synchronously; callers that must not
> block an async executor should run them on a blocking thread
> (e.g. `tokio::task::spawn_blocking`).
> Wallet implementation details (adapters, vendor differences, integration
> test checklist) are recorded in a separate integration-guide document, not
> in this design document, to preserve protocol-layer neutrality.

### 9.2 Three Connection Modes (by deployment)

| Mode | Connection | Suitable for | Status |
|---|---|---|---|
| **Local JSON-RPC** | same-host loopback HTTP JSON-RPC (wallet daemon provides `sign_message` / `sign_typed_data` etc.) | standalone self-host | **first release (P2)** |
| **In-process signer** | implement `WalletSigner` directly (reuse the host wallet's settlement implementation) | in-process / internal | first release (feature-gated, avoiding compile-time coupling) |
| **WC v2 (standard)** | LedgerFlow as a WalletConnect v2 client (dapp side) requesting signatures via the relay; methods: `personal_sign`, `eth_signTypedData_v4`, `solana_signMessage`, etc. + wallet-specific extensions | cross-process / remote / SaaS | **deferred past P2** (see risk #9) |

### 9.3 Signer Role Division

| Role | Who signs | Via |
|---|---|---|
| Warrant issuance | control plane / human (long-term key or wallet) | WC v2 / local RPC / server admin |
| PoP | agent (holder key, short-lived) | local signing in the agent runtime (not the wallet) |
| Approval (m-of-n) | approver (wallet holder) | wallet-signed message → `SignedApproval` (standard signing semantics + domain prefix) |
| On-chain payment (exact tx / UserOp / Solana tx) | agent or wallet | reuse the host wallet's settlement capability via `WalletSigner::sign_payment` |

---

## 10. SaaS Design

### 10.1 Three-Channel Model (from `saas_integration.md`)

| Channel | Notes |
|---|---|
| User JWT | browser → gateway; LedgerFlow **does not parse** |
| service token | gateway → LedgerFlow (`Authorization: Bearer`, constant-time comparison) |
| Internal headers | `x-internal-tenant-id` / `x-internal-user-id` / `x-internal-roles` / `x-internal-principal` → `SaaSContext` |

> `x-internal-roles` is relayed only for host-platform audit; LedgerFlow does
> **not consume** it (RBAC is out of scope, §4.3).

**Network-layer prerequisite (mandatory)**: `x-internal-*` trust relies on
"only the gateway can reach the service"; deployment docs MUST require mTLS /
network policy; the service must never be exposed on a direct public path.

### 10.2 LedgerFlow's SaaS Boundary

- `ledgerflow-server` provides the `[saas] mode = "standalone" | "saas"`
  configuration section; fail-fast semantics (§5.3);
- **Tenant isolation lands in three places**: ① warrant issuance keys / trust
  anchors isolated per tenant (§6.8); ② Facilitator settlement accounts and
  rail credentials isolated per tenant; ③ admin data (warrant/revocation/
  audit) filtered by `tenant_id`;
- **The Authz layer itself has no tenant concept** (stateless verification);
  tenants only affect the service layer — this keeps the core protocol
  generic;
- SaaS platform integration is not limited to LongCipher: the gateway
  internal-header protocol is a generic pattern; any gateway injecting
  equivalent headers works.

### 10.3 Open Integration Surface (standard APIs)

- **REST API** (utoipa/OpenAPI): warrant issuance, revocation, audit query,
  webhook subscription;
- **Webhooks**: payment success / settlement complete / approval-request
  events;
- **WC v2**: any wallet client can interact directly;
- **x402 / MPP**: any merchant and agent implementation interoperates.

---

## 11. Workspace Restructure (crate layout)

### 11.1 Target Layout

```text
ledgerflow/
├── bin/
│   ├── ledgerflow-cli/          # fixtures, issuance/revocation tooling, demos (kept)
│   └── ledgerflow-server/       # deployable service binary (new, phase 2)
├── crates/
│   ├── ledgerflow-core/         # Authz domain layer (kept + extended, purely stateless)
│   │   ├── warrant.rs           #  + delegation-chain fields/invariants
│   │   ├── delegation.rs        #  new: chain verification I1–I7 (runtime conjunction)
│   │   ├── approval.rs          #  new: SignedApproval / approval gates
│   │   ├── revocation.rs        #  new: RevocationCheck trait (pure interface, §11.2)
│   │   ├── trust.rs             #  new: TrustedIssuers anchor validation
│   │   ├── constraint.rs        #  kept (4 stateless constraint types; PeriodLimit removed)
│   │   ├── verification.rs      #  kept (typestate pipeline; approval/trust steps added)
│   │   └── ...                  #  proof_builder/typestate/error kept
│   ├── ledgerflow-protocol/     # transport-binding layer (evolved from ledgerflow-x402)
│   │   ├── x402/                #  extension codecs + MerchantVerifier middleware
│   │   ├── mpp/                 #  new: MPP challenge params + wrapper method
│   │   └── carrier/             #  generic carriers (MCP/A2A/header)
│   ├── ledgerflow-wallet/       # new: WalletSigner abstraction + local RPC / in-process adapters
│   ├── ledgerflow-facilitator/  # kept + enhanced (verify/settle/status + RevocationStore impl)
│   └── ledgerflow-server/       # new: REST / admin / webhook / SaaS mode (phase 2)
```

### 11.2 Relationship to Existing Code

| Existing | Disposition |
|---|---|
| `ledgerflow-core` (warrant/proof/constraint/typestate/verification) | **kept as the foundation**; adds delegation/approval/trust; `PeriodLimit` leaves the v1 constraint set |
| `ledgerflow-x402` (extension/middleware/replay) | evolves into `ledgerflow-protocol/x402` |
| `ledgerflow-facilitator` (routing/rails) | kept; adds verify orchestration, RevocationStore implementation, structured errors |
| `ledgerflow-cli` | kept; adds issuance/revocation commands |
| README/docs | rewritten for the protocol-layer positioning |

**Core principle (v0.2 revision)**: `ledgerflow-core` stays **purely
stateless** — `RevocationCheck` is defined in core as a trait (pure function
interface: `fn check(&self, warrant_id) -> Result<(), Revoked>`), and the
persistent implementation lives in `ledgerflow-facilitator` /
`ledgerflow-server`; core does no storage or networking.

### 11.3 Technology Choices (per AGENTS.md)

- HTTP: hpx (rustls) preferred; axum for Facilitator/server HTTP serving;
- Concurrency: scc (warrant cache / revocation tables), ArcSwap (trusted-issuers
  config hot updates);
- Storage: sqlx **0.9 stable or newer** (runtime queries) or plain files (MVP
  standalone may run without a database);
- Errors: thiserror in core, eyre at the application layer;
- API docs: utoipa; configuration: config crate + TOML;
- Observability: tracing + OTel OTLP.

---

## 12. Threat Model (new in v0.2)

> A security design document must explicitly state assets, attack surfaces,
> and blast radius. This is a condensed threat model; the full analysis ships
> with the spec document.

### 12.1 Assets

| Asset | Value | Protection |
|---|---|---|
| Holder private key (agent) | authorization invocation credential | local key management; PoP binding (stolen token unusable) |
| Issuer private key (control plane) | can issue arbitrary warrants | cold storage / HSM; key rotation (§6.8) |
| Approver private keys | m-of-n approval rights | wallet signatures; approval TTL |
| Facilitator settlement accounts | funds | least-privilege rail routing; settlement review |
| Revocation records | security commitment | persistence + atomic re-verification (§6.6, §8.1) |

### 12.2 Attack Surfaces and Blast Radius

| Attack | Scenario | Impact | Mitigation |
|---|---|---|---|
| Prompt-injection over-payment | agent injected with instructions | **max loss = warrant per-charge cap** (v1 has no periodic budget) | stateless constraints + PoP + approval gates |
| Warrant private-key leak | agent key stolen | holder-scoped authorization within the thief's control | PoP mandatory; short TTL; revocation |
| Issuance-key leak | control plane compromised | arbitrary warrant issuance (systemic) | cold storage, rotation, audit |
| Revocation-record loss | restart / failure | revoked warrants resurrect | persistence mandatory (§6.6) |
| Internal-header forgery | service directly exposed to the public net | tenant isolation broken | mTLS / network policy (§10.1); constant-time service-token check |
| Chain-node tampering | man-in-the-middle rewrites warrants | authorization scope expanded | per-node signature + parent_hash chain + I1–I7 |
| Clock-skew TTL/PoP bypass | time-server drift | expired tokens replayed | bidirectional clock-tolerance window (§6.3) |
| MPP session revocation delay | streaming payments | continued consumption inside the pre-revocation window | next-tick effect + active stream close (§6.6) |

### 12.3 Explicitly Not Protected

- A fully compromised agent process (RCE) — requires sidecar/gateway
  deployment isolation (deployment docs);
- Malicious tool implementations (authorization does not prevent tool bugs);
- A compromised control plane (root-of-trust problem);
- Side channels.

---

## 13. Testing and Quality Assurance (TDD + mutation testing)

> Project testing policy: **TDD (red-green-refactor) and mutation testing
> only; no BDD**. All behavior is driven by in-crate unit tests, integration
> tests (`tests/`), and `proptest` property tests; mutation testing validates
> test quality (`cargo mutants`).

### 13.1 TDD Workflow

- Every behavior change starts with a failing test (unit / integration), then
  implementation, then refactor;
- Behavior-level scenarios are expressed as integration tests (`tests/`)
  (see the acceptance behaviors below);
- Invariants use `proptest` property tests (run with `cargo test`).

### 13.2 Acceptance Behaviors (integration tests)

- `authorized payment`: agent with a warrant succeeds / without a warrant is
  rejected;
- `delegation attenuation`: child exceeding the parent's amount cap is
  rejected (I7 runtime conjunction);
- `approval gates`: high-value payments require m-of-n approvals; below
  threshold returns approval_required; retry succeeds;
- `revocation`: payments with a revoked warrant are rejected (verify
  pre-check + settle atomic re-check);
- `revocation persistence`: revoked warrants stay rejected after the
  Facilitator restarts;
- `replay`: the same PoP reused is rejected; challenge reuse is rejected;
- `trust anchors`: a chain whose root issuer is not in the trusted-issuers set
  is rejected;
- `clock skew`: PoP passes inside the tolerance window and is rejected outside;
- `config fail-fast`: invalid saas-mode configuration fails startup;
  standalone defaults work;
- `tenant isolation`: tenant A revoking a warrant does not affect tenant B
  (integration level);
- `chain tampering`: tampering with any node of the delegation chain fails
  verification (unit + end-to-end);
- `dual protocol`: the same warrant pays through both the x402 and MPP
  carriers.

### 13.3 Unit + Property Tests (proptest)

- Runtime conjunction: random multi-node constraint chains; per-node
  verification passes ⟺ whole chain allowed (invariant);
- Warrant CBOR deterministic codec round-trip + length cap;
- Delegation-chain verification: random chain lengths; tampering with any node
  fails (I5 domain-separated hash);
- Amount comparison: I7 monotonicity of the per-charge cap (random numerics);
- Signature verification: strict Ed25519 canonical verification (rejects
  non-canonical signatures).

### 13.4 Interop Tests

- **Cross-implementation**: LedgerFlow extensions interop with x402/MPP
  reference implementations (r402 / x402-rs / mpp-rs) on standard payloads
  (modeled on x402-rs's `protocol-conformance/` TS↔Rust pattern; MVP starts
  with Rust↔Rust fixture pair-checking); pair-check vectors include the PoP
  binding tuple's CBOR deterministic encoding and Ed25519 canonical
  signatures;
- **Wallet integration**: test against the first integrated wallet (local RPC
  - in-process signer modes).

### 13.5 Mutation Testing

- `just mutate` runs `cargo mutants` on the affected crates;
- Behavior-critical paths (verification pipeline, delegation invariants,
  constraint evaluation, PoP/approval signature checks, trust-anchor
  validation) must reach a survivable mutation score (each surviving mutant is
  reviewed; either add a test or rewrite the implementation);
- Mutation testing is not in CI (expensive); it is a manual pre-merge gate
  (`just mutate`).

### 13.6 Fuzzing and Benchmarks

- Fuzzing: keep and extend the existing `fuzz/` (warrant decoding, extension
  parsing, new approvals decoding);
- Benchmarks: keep the `ledgerflow-core` Criterion hot-path benchmark; add a
  delegation-chain verification benchmark;
- Full checks: `just format / lint / test / test-all / mutate`.

### 13.7 Audit Records (v0.2 wording revision)

- Revocation / approval / settlement events are written to an **audit record**
  (structured event stream);
- v1 provides an audit-query API (by warrant_id / holder / tenant);
- **Tamper-evident append-only log is on the roadmap** (reusing SRL design);
  v1 does not claim a "tamper-evident audit chain".

---

## 14. Phased Roadmap

| Phase | Content | Acceptance |
|---|---|---|
| **P0 foundation** (current → keep) | core 4 stateless constraint types + typestate + x402 extension codecs + replay | existing tests all green |
| **P1a delegation & PoP** | delegation chain I1–I7 (runtime conjunction) + PoP CBOR tuple + core stateless split + trust anchors | delegation/trust/clock/chain-tamper tests pass; fixture pair-checks pass |
| **P1b approvals & revocation** | approval gates + SignedApproval + RevocationCheck trait + persisted RevocationStore | approval/revocation/persistence/TOCTOU tests pass |
| **P1c MPP binding** | MPP challenge params + wrapper method + header size policy | dual-protocol interop tests pass |
| **P2 wallet & Facilitator** | `ledgerflow-wallet` (local RPC + in-process signer); Facilitator verify/settle orchestration + structured errors; accounting-point mode (periodic/cumulative budget) | integration tests with the first wallet pass; budget tests pass |
| **P3 deployable** | `ledgerflow-server` (REST/OpenAPI/webhook); standalone one-command run | self-host manual usable |
| **P4 SaaS** | saas mode (internal headers + service token + tenant isolation + mTLS); gateway integration docs | SaaS platform integration demo |
| **P5 ecosystem** (on demand) | SRL revocation broadcast; WC v2 client adapter; Secp256k1 warrant signing; WASM binding; more rails (Tempo/Stripe); paymaster sponsored payments | community-PR friendly |

> Deliberately deferred: multi-language SDK matrix, orchestrator, CEL
> constraints, tamper-evident audit chain — outside the MVP scope.

---

## 15. Risks and Open Questions

| # | Risk / question | Response |
|---|---|---|
| 1 | x402 extensions do not cover all transports (e.g., MCP v2 carrier differences) | align with x402-foundation; carrier abstraction as fallback (§7.3) |
| 2 | **Permission for clients to "add" extensions in PaymentPayload extensions is unconfirmed** (server-advertised extensions are protected by "no delete/overwrite"; whether client-added keys are fail-closed-rejected by implementations is unknown) | confirm with x402-foundation before freezing the spec; fallback: all LedgerFlow data under a single `ledgerflow` key |
| 3 | MPP auth-scheme spec is still an IETF draft; challenge-param extensions may be affected by spec evolution | `ledgerflow-` prefix on extension params; track draft changes |
| 4 | Revocation eventual-consistency window (pre-SRL) | documented for MVP (window is zero under a single accounting point); SRL mitigates later |
| 5 | **Revocation-record loss across restarts (closed by §6.6 persistence mandate)**; in-memory mode is demo-only and explicitly declared | persistence mandatory in production; standalone demo `--insecure-revoc-memory` |
| 6 | standalone database-free boundary | explicit no-DB for P0–P2; optional sqlx (stable) at P3 server |
| 7 | tension between deep wallet integration and genericity | everything goes through the `WalletSigner` trait / standard protocols; integration lives only in the adapter layer (§9) |
| 8 | version compatibility after v1 extension freeze | wire format carries version + extensions map (frozen v1, unknown keys rejected); backward-compat policy |
| 9 | **thin Rust-side WC v2 client ecosystem** (mostly JS/Swift/Kotlin SDKs); starting P2 with WC v2 as the first citizen may be blocked by the library | **P2 ships local RPC + in-process signer first**; WC v2 moves to P5 with a self-built lightweight relay evaluation item |
| 10 | gasless / sponsored payment interaction with the authz layer | `SponsorshipConstraint` deferred together with paymaster (§4.3, §6.4) |
| 11 | approver key rotation | documented as a v1 limitation (fixed key set at issuance); key rotation on the roadmap (§6.5) |
| 12 | availability single point of the budget accounting point (P2+) | accounting point declared by the warrant (`ledgerflow.ledger`); agents cannot overdraw across accounting points; high availability of the accounting point is a deployment responsibility |

---

## 16. Glossary

| Term | Definition |
|---|---|
| Warrant | signed capability token: who (holder) is authorized by whom (issuer) to do what (constraints), when (TTL), and how much (per-charge cap) |
| PoP | Proof-of-Possession: proves the caller holds the private key bound to the warrant |
| Attenuation | delegation only narrows, never expands (implemented via runtime conjunction) |
| Approval Gate | a call pattern (tool+args) that requires a human signature confirmation |
| SRL | Signed Revocation List (roadmap) |
| RevocationStore | online revocation store (production persistence mandatory) |
| Facilitator | the intermediate service that verifies payments and routes settlement (incl. online revocation checks) |
| Rail | settlement rail: EVM / Solana / Tempo / Stripe / gateway, etc. |
| Verifier Model | the verification model: which checks are offline (stateless) vs online (budget/revocation) |
| Accounting Point | the single Facilitator enforcing periodic/cumulative budget (bound by the warrant) |
| SaaS mode | multi-tenant deployment with identity and tenant context injected by a gateway |
| Trusted Issuers | the set of trusted issuer public keys configured by merchants/verifiers (trust anchors) |
