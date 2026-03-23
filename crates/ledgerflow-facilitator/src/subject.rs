//! Payment-subject resolution for Facilitator routing.

use ledgerflow_core::{PaymentSubjectKind, VerifiedAuthorization};
use thiserror::Error;

use crate::routing::RailKind;

/// Subject information normalized for routing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSubject {
    pub rail: RailKind,
    pub value: String,
}

/// Subject resolution failures.
#[derive(Debug, Error)]
pub enum SubjectResolutionError {
    #[error("payment subject `{value}` could not be resolved to a known rail")]
    UnsupportedSubject { value: String },
}

/// Resolves a verified payment subject to a rail hint.
pub trait PaymentSubjectResolver {
    fn resolve(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<ResolvedSubject, SubjectResolutionError>;
}

/// Default subject resolver for the MVP's onchain, exchange, custodial, and gateway rails.
#[derive(Clone, Debug, Default)]
pub struct DefaultSubjectResolver;

impl PaymentSubjectResolver for DefaultSubjectResolver {
    fn resolve(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<ResolvedSubject, SubjectResolutionError> {
        let subject = &authorization.payment_subject;
        let rail = match subject.kind {
            PaymentSubjectKind::Caip10 => RailKind::Evm,
            PaymentSubjectKind::ExchangeAccount => RailKind::Exchange,
            PaymentSubjectKind::FacilitatorAccount if subject.value.starts_with("binance:") => {
                RailKind::Exchange
            }
            PaymentSubjectKind::FacilitatorAccount if subject.value.starts_with("okx:") => {
                RailKind::Exchange
            }
            PaymentSubjectKind::Opaque if subject.value.starts_with("gateway:") => {
                RailKind::Gateway
            }
            PaymentSubjectKind::Opaque => RailKind::Custodial,
            PaymentSubjectKind::FacilitatorAccount => RailKind::Exchange,
            _ => {
                return Err(SubjectResolutionError::UnsupportedSubject {
                    value: subject.value.clone(),
                });
            }
        };

        Ok(ResolvedSubject { rail, value: subject.value.clone() })
    }
}
