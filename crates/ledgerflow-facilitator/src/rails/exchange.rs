//! Exchange rail adapter for offchain account settlement.

use crate::{rails::RailAdapter, routing::RailKind, subject::ResolvedSubject};

/// Adapter for exchange or gateway-style settlement.
#[derive(Clone, Copy, Debug)]
pub struct ExchangeRailAdapter;

impl RailAdapter for ExchangeRailAdapter {
    fn kind(&self) -> RailKind {
        RailKind::Exchange
    }

    fn supports(&self, subject: &ResolvedSubject) -> bool {
        subject.rail == RailKind::Exchange
    }
}
