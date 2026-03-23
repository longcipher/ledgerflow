//! Development CLI for local LedgerFlow fixtures.

#![allow(clippy::print_stdout)]

use clap::{Parser, Subcommand};
use eyre::{OptionExt, Result};
use ledgerflow_core::{
    AmountLimit, AssetRef, AudienceScope, Constraint, DelegationPolicy, MerchantConstraint,
    PaymentConstraint, PaymentRail, PaymentSubjectKind, PaymentSubjectRef, ResourceConstraint,
    SignerRef, SigningAlgorithm, SponsorshipConstraint, ToolConstraint, WARRANT_VERSION_V1,
    Warrant, WarrantMetadata,
};
use ledgerflow_x402::{
    AcceptedQuote, HttpRequest, PaymentPayloadSeed, WarrantTransport, build_payment_payload,
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
    };

    Ok(output)
}

fn render_sample_warrant_fixture() -> String {
    let warrant = sample_warrant();
    format!(
        "warrant_id={}\nmerchant_id=merchant-a\ntool_name=web-search\namount=200\npayment_subject={}\ndigest={}",
        warrant.warrant_id,
        warrant.payment_subjects[0].value,
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
        WarrantTransport::inline(sample_warrant()),
        PaymentPayloadSeed {
            payment_subject: sample_subject(),
            signer: SignerRef::new(SigningAlgorithm::Ed25519, "agent-key"),
            created_at_ms: 2_000,
            nonce: "nonce-1".to_string(),
            payment_identifier: Some("payment-1".to_string()),
        },
    );
    let extension = payload.ledgerflow.ok_or_eyre("missing sample payment ledgerflow extension")?;
    let payment_identifier =
        payload.payment_identifier.as_deref().ok_or_eyre("missing sample payment identifier")?;

    Ok(format!(
        "challenge_id={}\npayment_identifier={}\naccepted_amount={}\nwarrant_digest={}\nrequest_hash={}\naccepted_hash={}\npayment_subject={}",
        extension.challenge_id,
        payment_identifier,
        payload.accepted.amount,
        extension.warrant.digest,
        extension.proof.request_hash,
        extension.proof.accepted_hash,
        extension.payment_subject.value,
    ))
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

fn sample_warrant() -> Warrant {
    Warrant {
        version: WARRANT_VERSION_V1,
        warrant_id: "warrant-1".to_string(),
        issuer: SignerRef::new(SigningAlgorithm::Ed25519, "issuer-key"),
        subject_signer: SignerRef::new(SigningAlgorithm::Ed25519, "agent-key"),
        payment_subjects: vec![sample_subject()],
        audience: AudienceScope::MerchantIds(vec!["merchant-a".to_string()]),
        not_before_ms: 1_000,
        expires_at_ms: 10_000,
        delegation: DelegationPolicy { can_delegate: true, max_depth: 1 },
        constraints: vec![
            Constraint::Merchant(MerchantConstraint {
                merchant_ids: vec!["merchant-a".to_string()],
                host_suffixes: vec![],
            }),
            Constraint::Resource(ResourceConstraint {
                http_methods: vec!["POST".to_string()],
                path_prefixes: vec!["/pay".to_string()],
            }),
            Constraint::Tool(ToolConstraint {
                tool_names: vec!["web-search".to_string()],
                model_providers: vec![],
                action_labels: vec![],
            }),
            Constraint::Payment(PaymentConstraint {
                max_per_request: AmountLimit { amount: 200 },
                period_limit: None,
                allowed_assets: vec![AssetRef::new("USDC", Some("base".to_string()))],
                allowed_rails: vec![PaymentRail::Onchain],
                allowed_schemes: vec!["exact".to_string()],
                payee_ids: vec!["merchant-a".to_string()],
            }),
            Constraint::Sponsorship(SponsorshipConstraint {
                allow_sponsored_execution: false,
                sponsor_ids: vec![],
            }),
        ],
        metadata: WarrantMetadata::default(),
        signature: SignerRef::new(SigningAlgorithm::Ed25519, "issuer-key")
            .sign_message("placeholder"),
    }
    .sign()
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, render_sample_payment_fixture, render_sample_warrant_fixture, run};

    const SAMPLE_WARRANT_FIXTURE: &str =
        include_str!("../../../crates/ledgerflow-x402/tests/fixtures/sample-warrant.txt");
    const SAMPLE_PAYMENT_FIXTURE: &str =
        include_str!("../../../crates/ledgerflow-x402/tests/fixtures/sample-payment.txt");

    #[test]
    fn clap_command_configuration_is_valid() {
        <Cli as clap::CommandFactory>::command().debug_assert();
    }

    #[test]
    fn sample_warrant_fixture_mentions_core_fields() {
        let output = render_sample_warrant_fixture();

        assert_eq!(output, SAMPLE_WARRANT_FIXTURE.trim_end());
    }

    #[test]
    fn sample_payment_fixture_mentions_extension_fields() {
        let output = render_sample_payment_fixture().expect("payment fixture");

        assert_eq!(output, SAMPLE_PAYMENT_FIXTURE.trim_end());
    }

    #[test]
    fn run_returns_fixture_text_for_each_subcommand() {
        let warrant_output = run(Command::SampleWarrant).expect("warrant output");
        let payment_output = run(Command::SamplePayment).expect("payment output");

        assert!(warrant_output.contains("warrant_id="));
        assert!(payment_output.contains("challenge_id="));
    }
}
