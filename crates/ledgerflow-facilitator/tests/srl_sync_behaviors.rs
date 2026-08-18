//! Integration tests for SRL synchronization onto a persistent revocation
//! store (multi-node SaaS revocation).

#![allow(clippy::expect_used)]

use ledgerflow_core::{
    RevocationCheck, RevocationDecision, SignedRevocationList, SrlEntry, hex_encode_bytes,
};
use ledgerflow_facilitator::{FileRevocationStore, SrlSync};

fn control_keys() -> ledgerflow_core::SigningKeyPair {
    ledgerflow_core::SigningKeyPair::from_bytes(&[0x55; 32])
}

fn holder_keys() -> ledgerflow_core::SigningKeyPair {
    ledgerflow_core::SigningKeyPair::from_bytes(&[0x66; 32])
}

#[test]
fn srl_apply_persists_revocations_to_store() {
    let dir = std::env::temp_dir().join(format!("ledgerflow-srl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("revocations.jsonl");
    let _ = std::fs::remove_file(&path);

    let store = FileRevocationStore::open(&path).expect("open");
    let sync = SrlSync::new(store.clone(), control_keys().signer_ref());
    assert_eq!(sync.applied_version(), 0);

    let warrant_id = [0xAA; 16];
    let list = SignedRevocationList::sign(
        1,
        vec![SrlEntry::Warrant { id_hex: hex_encode_bytes(&warrant_id) }],
        &control_keys(),
    );
    sync.apply(&list).expect("apply");

    assert_eq!(sync.applied_version(), 1);
    assert_eq!(store.check_warrant(&warrant_id), RevocationDecision::RevokedWarrant);
    // Persisted across restart.
    let reloaded = FileRevocationStore::open(&path).expect("reopen");
    assert_eq!(reloaded.check_warrant(&warrant_id), RevocationDecision::RevokedWarrant);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn srl_apply_rejects_rollback_and_bad_sig() {
    let dir = std::env::temp_dir().join(format!("ledgerflow-srl-rb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("revocations.jsonl");
    let _ = std::fs::remove_file(&path);

    let store = FileRevocationStore::open(&path).expect("open");
    let sync = SrlSync::new(store.clone(), control_keys().signer_ref());

    sync.apply(&SignedRevocationList::sign(
        2,
        vec![SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xAA; 16]) }],
        &control_keys(),
    ))
    .expect("v2");

    // Rollback to v1 rejected.
    let rollback = sync.apply(&SignedRevocationList::sign(
        1,
        vec![SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xBB; 16]) }],
        &control_keys(),
    ));
    assert!(rollback.is_err(), "anti-rollback must reject older version");

    // Forged signature rejected.
    let attacker = ledgerflow_core::SigningKeyPair::from_bytes(&[0x77; 32]);
    let forged = sync.apply(&SignedRevocationList::sign(
        3,
        vec![SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xCC; 16]) }],
        &attacker,
    ));
    assert!(forged.is_err(), "forged signature must be rejected");
    // The forged entry was NOT persisted.
    assert_eq!(store.check_warrant(&[0xCC; 16]), RevocationDecision::Ok);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn srl_apply_holder_revocation_persists() {
    let dir = std::env::temp_dir().join(format!("ledgerflow-srl-h-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("revocations.jsonl");
    let _ = std::fs::remove_file(&path);

    let store = FileRevocationStore::open(&path).expect("open");
    let sync = SrlSync::new(store.clone(), control_keys().signer_ref());
    let holder = holder_keys().signer_ref();

    sync.apply(&SignedRevocationList::sign(
        1,
        vec![SrlEntry::Holder { key_hex: hex_encode_bytes(&holder.public_key) }],
        &control_keys(),
    ))
    .expect("apply holder");

    assert_eq!(store.check_holder(&holder), RevocationDecision::RevokedHolder);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}
