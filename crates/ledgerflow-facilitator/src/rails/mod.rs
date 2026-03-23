//! Rail adapters supported by the MVP Facilitator.

pub mod evm;
pub mod exchange;

use crate::{routing::RailKind, subject::ResolvedSubject};

/// Trait implemented by each settlement rail adapter.
pub trait RailAdapter {
    fn kind(&self) -> RailKind;
    fn supports(&self, subject: &ResolvedSubject) -> bool;
}
