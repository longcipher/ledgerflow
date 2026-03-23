//! EVM rail adapter for CAIP-10 subjects.

use crate::{rails::RailAdapter, routing::RailKind, subject::ResolvedSubject};

/// Adapter for EVM settlement.
#[derive(Clone, Copy, Debug)]
pub struct EvmRailAdapter;

impl RailAdapter for EvmRailAdapter {
    fn kind(&self) -> RailKind {
        RailKind::Evm
    }

    fn supports(&self, subject: &ResolvedSubject) -> bool {
        subject.rail == RailKind::Evm
    }
}
