#![no_main]

use ledgerflow_core::Warrant;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = Warrant::decode_cbor(data);
});
