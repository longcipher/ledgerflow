//! Development CLI for LedgerFlow fixtures and tooling.

#![allow(clippy::print_stdout)]

use clap::{Parser, Subcommand};
use eyre::{OptionExt, Result};
use ledgerflow_core::{
    ApprovalGate, AssetRef, MerchantConstraint, PaymentConstraint, PaymentRail, PaymentSubjectKind,
    PaymentSubjectRef, ResourceConstraint, SignedApproval, SigningKeyPair, TrustedIssuer,
    TrustedIssuers, WarrantBuilder, WarrantChain,
};
use ledgerflow_protocol::{
    AcceptedQuote, HttpRequest, PaymentPayloadSeed, build_payment_payload,
    merchant_payment_required,
};

#[derive(Debug, Parser)]
#[command(name = "ledgerflow-cli", version, about = "Development commands for LedgerFlow fixtures")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print a deterministic sample warrant fixture.
    SampleWarrant,
    /// Print a deterministic sample x402 payment payload with LedgerFlow authz data.
    SamplePayment,
    /// Sign an m-of-n approval for a request hash (for demos).
    Approve {
        /// The request hash to approve.
        request_hash: String,
        /// Approver secret key hex (64 hex chars = 32 bytes).
        #[arg(long)]
        secret_hex: Option<String>,
    },
    /// Show the trusted-issuer anchor configuration hint.
    TrustAnchors,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    println!("{}", run(cli.command)?);
    Ok(())
}

fn run(command: Command) -> Result<String> {
    let output = match command {
        Command::SampleWarrant => render_sample_warrant_fixture(),
        Command::SamplePayment => render_sample_payment_fixture()?,
        Command::Approve { request_hash, secret_hex } => {
            render_approval(&request_hash, secret_hex.as_deref())
        }
        Command::TrustAnchors => render_trust_anchors(),
    };

    Ok(output)
}

fn render_sample_warrant_fixture() -> String {
    let warrant = sample_warrant();
    format!(
        "warrant_id={}\nmerchant_id=merchant-a\ntool_name=web-search\namount=200\npayment_subject={}\ndigest={}",
        warrant.id_hex(),
        warrant.merchant.merchant_ids.first().map_or("-", String::as_str),
        warrant.digest(),
    )
}

fn render_sample_payment_fixture() -> Result<String> {
    let request = sample_request();
    let challenge = merchant_payment_required(
        "challenge-1",
        "merchant-a",
        "/pay",
        vec![sample_quote()],
        60_000,
    )
    .ledgerflow
    .ok_or_eyre("missing sample challenge extension")?;
    let payload = build_payment_payload(
        &challenge,
        &request,
        sample_quote(),
        WarrantChain::single(sample_warrant()),
        PaymentPayloadSeed {
            payment_subject: sample_subject(),
            signer: agent_keys(),
            created_at_ms: 2_000,
            nonce: "nonce-1".to_string(),
            payment_identifier: Some("payment-1".to_string()),
            tool_args: std::collections::BTreeMap::new(),
            approvals: Vec::new(),
        },
    )?;
    let extension = payload.ledgerflow.ok_or_eyre("missing sample payment ledgerflow extension")?;
    let payment_identifier =
        payload.payment_identifier.as_deref().ok_or_eyre("missing sample payment identifier")?;
    let warrant_digest = match extension.warrant_chain.last() {
        Some(warrant) => warrant.digest(),
        None => "-".to_string(),
    };

    Ok(format!(
        "challenge_id={}\npayment_identifier={}\naccepted_amount={}\nwarrant_digest={}\nrequest_hash={}\naccepted_hash={}\npayment_subject={}",
        extension.challenge_id,
        payment_identifier,
        payload.accepted.amount,
        warrant_digest,
        extension.proof.tuple.request_hash,
        extension.proof.tuple.accepted_hash,
        extension.payment_subject.value,
    ))
}

fn render_approval(request_hash: &str, secret_hex: Option<&str>) -> String {
    let approver = match secret_hex {
        Some(hex) => SigningKeyPair::from_bytes(&hex_bytes(hex)),
        None => approver_keys(),
    };
    const DEFAULT_APPROVAL_TTL_SECS: u64 = 300;
    let approval = SignedApproval::sign(
        request_hash,
        &approver.signer_ref(),
        DEFAULT_APPROVAL_TTL_SECS,
        &approver,
    );
    format!(
        "request_hash={}\napprover={}\nexpires_at={}\nsignature_hex={}",
        approval.request_hash,
        hex_encode(&approval.approver.public_key),
        approval.expires_at,
        hex_encode(&approval.signature.value),
    )
}

fn render_trust_anchors() -> String {
    let issuer = issuer_keys();
    let mut set = TrustedIssuers::new();
    set.add(TrustedIssuer::new("issuer-1".to_string(), issuer.signer_ref()));
    let anchor = &set.issuers[0];
    format!(
        "key_id={}\npublic_key_hex={}\nalg={}",
        anchor.key_id,
        hex_encode(&anchor.issuer.public_key),
        anchor.issuer.alg.as_str(),
    )
}

fn sample_request() -> HttpRequest {
    HttpRequest::new("POST", "merchant-a.example", "/pay", br#"{"ok":true}"#.to_vec())
}

fn sample_quote() -> AcceptedQuote {
    AcceptedQuote::exact("USDC", 200, "merchant-a", Some("base".to_string()))
}

fn sample_subject() -> PaymentSubjectRef {
    PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123")
}

fn issuer_keys() -> SigningKeyPair {
    let secret: [u8; 32] = *b"issuer-secret-key-32-bytes-long!";
    SigningKeyPair::from_bytes(&secret)
}

fn agent_keys() -> SigningKeyPair {
    let secret: [u8; 32] = *b"agent-secret-key--32-bytes-long!";
    SigningKeyPair::from_bytes(&secret)
}

fn approver_keys() -> SigningKeyPair {
    let secret: [u8; 32] = *b"approver-key-32-bytes-long!00000";
    SigningKeyPair::from_bytes(&secret)
}

fn sample_warrant() -> ledgerflow_core::Warrant {
    let issuer = issuer_keys();
    let holder = agent_keys();
    WarrantBuilder::new(2_000)
        .warrant_id(*b"lfw-000000000001")
        .ttl_secs(10)
        .max_depth(1)
        .issuer(issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
        .resource(ResourceConstraint {
            http_methods: vec!["POST".to_string()],
            path_prefixes: vec!["/pay".to_string()],
        })
        .payment(
            PaymentConstraint::new(200)
                .with_asset(AssetRef::new("USDC", Some("base".to_string())))
                .with_rails(vec![PaymentRail::Onchain])
                .with_schemes(vec!["exact".to_string()])
                .with_payees(vec!["merchant-a".to_string()]),
        )
        .approval_gate("web-search", ApprovalGate::unconditional())
        .sign_with(&issuer, [0_u8; 8])
}

fn hex_bytes(hex: &str) -> [u8; 32] {
    let mut out = [0_u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).take(32).enumerate() {
        let text = std::str::from_utf8(chunk).unwrap_or("00");
        out[i] = u8::from_str_radix(text, 16).unwrap_or(0);
    }
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0F) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{Cli, Command, run};

    #[test]
    fn clap_command_configuration_is_valid() {
        <Cli as clap::CommandFactory>::command().debug_assert();
    }

    #[test]
    fn sample_warrant_fixture_produces_deterministic_output() {
        let first = super::render_sample_warrant_fixture();
        let second = super::render_sample_warrant_fixture();
        assert_eq!(first, second);
        assert!(first.contains("warrant_id="));
        assert!(first.contains("digest=sha256:"));
    }

    #[test]
    fn sample_payment_fixture_produces_deterministic_output() {
        let first = super::render_sample_payment_fixture().expect("payment fixture");
        let second = super::render_sample_payment_fixture().expect("payment fixture");
        assert_eq!(first, second);
        assert!(first.contains("challenge_id=challenge-1"));
        assert!(first.contains("payment_identifier=payment-1"));
    }

    #[test]
    fn approval_fixture_is_deterministic() {
        let first = super::render_approval("sha256:request", None);
        let second = super::render_approval("sha256:request", None);
        assert_eq!(first, second);
        assert!(first.contains("request_hash=sha256:request"));
    }

    #[test]
    fn trust_anchor_output_is_deterministic() {
        let first = super::render_trust_anchors();
        let second = super::render_trust_anchors();
        assert_eq!(first, second);
        assert!(first.contains("key_id=issuer-1"));
    }

    #[test]
    fn run_returns_fixture_text_for_each_subcommand() {
        let warrant_output = run(Command::SampleWarrant).expect("warrant output");
        let payment_output = run(Command::SamplePayment).expect("payment output");
        assert!(warrant_output.contains("warrant_id="));
        assert!(payment_output.contains("challenge_id="));
    }

    #[test]
    fn hex_bytes_and_hex_encode_round_trip() {
        let source = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let bytes = super::hex_bytes(source);
        assert_eq!(super::hex_encode(&bytes), source);

        // Odd-length input is padded/truncated defensively (32 bytes max).
        let short = super::hex_bytes("aabb");
        assert_eq!(super::hex_encode(&short[..2]), "aabb");
        // Invalid hex falls back to zero.
        let invalid = super::hex_bytes("zzzz");
        assert_eq!(invalid[0], 0);
        assert_eq!(invalid[1], 0);
    }

    #[test]
    fn approval_fixture_with_explicit_secret_is_deterministic() {
        let secret_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let first = super::render_approval("sha256:req", Some(secret_hex));
        let second = super::render_approval("sha256:req", Some(secret_hex));
        assert_eq!(first, second);
        // The approver is the Ed25519 public key DERIVED from the secret, so
        // it is not the raw secret bytes; the line still has the right shape.
        assert!(first.contains("request_hash=sha256:req"));
        assert!(first.contains("approver="));
        assert!(first.contains("signature_hex="));
    }
}
