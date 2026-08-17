#![no_main]

use ledgerflow_protocol::LedgerFlowAuthorizationExtension;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = LedgerFlowAuthorizationExtension::decode_cbor(data);
});
