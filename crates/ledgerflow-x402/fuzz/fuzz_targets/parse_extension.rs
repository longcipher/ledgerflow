#![no_main]

use libfuzzer_sys::fuzz_target;
use ledgerflow_x402::{LedgerFlowAuthorizationExtension, LedgerFlowChallenge};

fuzz_target!(|data: &[u8]| {
    let _ = LedgerFlowChallenge::decode_cbor(data);
    let _ = LedgerFlowAuthorizationExtension::decode_cbor(data);
});
