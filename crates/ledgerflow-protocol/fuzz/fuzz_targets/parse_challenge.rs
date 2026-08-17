#![no_main]

use ledgerflow_protocol::LedgerFlowChallenge;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = LedgerFlowChallenge::decode_cbor(data);
});
