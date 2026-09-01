//! WalletConnect v2 crypto primitives.
//!
//! Implements the WC v2 key-agreement and envelope scheme exactly as the
//! official `@walletconnect/utils` reference implementation:
//!
//! - **Key agreement**: X25519 ECDH → 32-byte shared secret.
//! - **KDF**: HKDF-SHA256 with `ikm = shared_secret`, a 32-zero-byte salt, empty `info`, and a
//!   32-byte output (`deriveSymKey`).
//! - **Cipher**: ChaCha20-Poly1305 (IETF construction, 12-byte nonce), empty AAD.
//! - **Envelopes** (serialized then base64-encoded by the relay layer):
//!   - Type 0: `[type(1) ‖ iv(12) ‖ ciphertext]` — anonymous encrypted envelope.
//!   - Type 1: `[type(1) ‖ senderPubKey(32) ‖ iv(12) ‖ ciphertext]` — carries the sender's X25519
//!     public key so the receiver can derive the session key.
//!   - Type 2: `[type(1) ‖ plaintext]` — unencrypted.

use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use hkdf::Hkdf;
use rand::RngExt;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::error::WalletError;

/// Envelope type byte for an anonymous encrypted envelope (type 0).
pub(crate) const ENVELOPE_TYPE_0: u8 = 0;
/// Envelope type byte for an authenticated sender envelope (type 1).
pub(crate) const ENVELOPE_TYPE_1: u8 = 1;
/// Envelope type byte for an unencrypted envelope (type 2).
pub(crate) const ENVELOPE_TYPE_2: u8 = 2;

/// Nonce length for ChaCha20-Poly1305 (IETF standard: 96 bits).
pub(crate) const IV_LENGTH: usize = 12;
/// X25519 key length (bytes).
pub(crate) const KEY_LENGTH: usize = 32;

/// X25519 keypair for WC v2 session key agreement.
#[derive(Clone)]
pub(crate) struct WcKeyPair {
    secret: StaticSecret,
    public: X25519PublicKey,
}

impl WcKeyPair {
    /// Generates a fresh X25519 keypair.
    pub(crate) fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::rng().fill(&mut bytes[..]);
        let secret = StaticSecret::from(bytes);
        let public = X25519PublicKey::from(&secret);
        Self { secret, public }
    }

    /// Returns the public key.
    #[allow(dead_code)] // used by tests and future session-key derivation paths
    pub(crate) const fn public_key(&self) -> X25519PublicKey {
        self.public
    }

    /// Hex-encoded public key (32 bytes → 64 hex chars), matching the
    /// official client's `BASE16` output.
    pub(crate) fn public_key_hex(&self) -> String {
        hex::encode(self.public.as_bytes())
    }

    /// Derives the shared secret with the peer's public key.
    pub(crate) fn shared_secret(&self, peer: &X25519PublicKey) -> WcSharedSecret {
        let s = self.secret.diffie_hellman(peer);
        WcSharedSecret(Zeroizing::new(s.to_bytes()))
    }
}

/// Shared secret derived from X25519.
pub(crate) struct WcSharedSecret(Zeroizing<[u8; 32]>);

impl WcSharedSecret {
    /// Returns the raw shared-secret bytes.
    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// 256-bit symmetric key (for ChaCha20-Poly1305 message encryption).
#[derive(Clone)]
pub(crate) struct WcSymKey(Zeroizing<[u8; 32]>);

impl WcSymKey {
    /// Constructs a key from raw bytes.
    pub(crate) fn from_bytes(b: [u8; 32]) -> Self {
        Self(Zeroizing::new(b))
    }

    /// Generates a random key.
    #[allow(dead_code)] // used by tests and future envelope helpers
    pub(crate) fn from_random() -> Self {
        let mut b = [0u8; 32];
        rand::rng().fill(&mut b[..]);
        Self(Zeroizing::new(b))
    }

    /// Returns the raw key bytes.
    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Derives the shared symmetric key from an X25519 shared secret, exactly as
/// the official `@walletconnect/utils` `deriveSymKey` does:
///
/// ```text
/// HKDF-SHA256(ikm = shared_secret, salt = zeroes(32), info = "", len = 32)
/// ```
///
/// The official TypeScript implementation calls
/// `hkdf(sha256, sharedKey, undefined, undefined, KEY_LENGTH)` where
/// `@noble/hashes` treats `undefined` salt as a zero-filled array of the hash
/// output length (32 bytes) and `undefined` info as an empty array.
pub(crate) fn derive_sym_key(shared_secret: &WcSharedSecret) -> Result<WcSymKey, WalletError> {
    let salt = [0u8; 32];
    let hk = Hkdf::<Sha256>::new(Some(&salt), shared_secret.as_bytes());
    let mut okm = [0u8; KEY_LENGTH];
    // KEY_LENGTH (32) is within HKDF-SHA256's 255*32 output limit, so expand
    // cannot fail (RFC 5869); the error is still surfaced defensively.
    hk.expand(&[], &mut okm)
        .map_err(|e| WalletError::Transport(format!("hkdf expand failed: {e}")))?;
    Ok(WcSymKey::from_bytes(okm))
}

/// SHA-256 of raw bytes, hex-encoded.
pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Stateless ChaCha20-Poly1305 AEAD wrapper matching the official
/// `@walletconnect/utils` `encrypt`/`decrypt` (no AAD).
pub(crate) struct WcCipher;

impl WcCipher {
    /// ChaCha20-Poly1305 AEAD seal with **empty AAD** (official behavior).
    fn seal(key: &WcSymKey, nonce: &[u8; 12], plaintext: &[u8]) -> Result<Vec<u8>, WalletError> {
        let cipher = ChaCha20Poly1305::new_from_slice(key.as_bytes())
            .map_err(|e| WalletError::Transport(format!("invalid key: {e}")))?;
        let n = Nonce::try_from(nonce.as_slice())
            .map_err(|e| WalletError::Transport(format!("invalid nonce: {e}")))?;
        cipher
            .encrypt(&n, Payload { msg: plaintext, aad: &[] })
            .map_err(|e| WalletError::Transport(format!("encrypt failed: {e}")))
    }

    /// ChaCha20-Poly1305 AEAD open with **empty AAD**.
    fn open(
        key: &WcSymKey,
        nonce: &[u8; 12],
        ciphertext_with_tag: &[u8],
    ) -> Result<Vec<u8>, WalletError> {
        let cipher = ChaCha20Poly1305::new_from_slice(key.as_bytes())
            .map_err(|e| WalletError::Transport(format!("invalid key: {e}")))?;
        let n = Nonce::try_from(nonce.as_slice())
            .map_err(|e| WalletError::Transport(format!("invalid nonce: {e}")))?;
        cipher
            .decrypt(&n, Payload { msg: ciphertext_with_tag, aad: &[] })
            .map_err(|e| WalletError::Transport(format!("decrypt failed: {e}")))
    }

    /// Encrypts a plaintext into a **type-0 envelope**:
    /// `[0x00 ‖ iv(12) ‖ ciphertext ‖ tag(16)]`.
    pub(crate) fn seal_type0(key: &WcSymKey, plaintext: &[u8]) -> Result<Vec<u8>, WalletError> {
        let mut iv = [0u8; IV_LENGTH];
        rand::rng().fill(&mut iv[..]);
        let sealed = Self::seal(key, &iv, plaintext)?;
        Ok(serialize_envelope(ENVELOPE_TYPE_0, &iv, None, &sealed))
    }

    /// Decrypts a type-0 envelope.
    pub(crate) fn open_type0(key: &WcSymKey, envelope: &[u8]) -> Result<Vec<u8>, WalletError> {
        let env = deserialize_envelope(envelope)?;
        if env.r#type != ENVELOPE_TYPE_0 {
            return Err(WalletError::Transport(format!(
                "expected type-0 envelope, got type-{}",
                env.r#type
            )));
        }
        Self::open(key, &env.iv, &env.sealed)
    }

    /// Encrypts a plaintext into a **type-1 envelope** carrying the sender's
    /// X25519 public key: `[0x01 ‖ senderPubKey(32) ‖ iv(12) ‖ ciphertext]`.
    pub(crate) fn seal_type1(
        key: &WcSymKey,
        sender_pubkey: &[u8; 32],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, WalletError> {
        let mut iv = [0u8; IV_LENGTH];
        rand::rng().fill(&mut iv[..]);
        let sealed = Self::seal(key, &iv, plaintext)?;
        Ok(serialize_envelope(ENVELOPE_TYPE_1, &iv, Some(sender_pubkey), &sealed))
    }

    /// Decrypts a type-1 envelope (returns the sender's public key + plaintext).
    pub(crate) fn open_type1(
        key: &WcSymKey,
        envelope: &[u8],
    ) -> Result<([u8; 32], Vec<u8>), WalletError> {
        let env = deserialize_envelope(envelope)?;
        if env.r#type != ENVELOPE_TYPE_1 {
            return Err(WalletError::Transport(format!(
                "expected type-1 envelope, got type-{}",
                env.r#type
            )));
        }
        let sender = env.sender_public_key.ok_or_else(|| {
            WalletError::Transport("type-1 envelope missing sender key".to_string())
        })?;
        let plaintext = Self::open(key, &env.iv, &env.sealed)?;
        Ok((sender, plaintext))
    }
}

/// A deserialized WC v2 message envelope.
#[derive(Debug, Clone)]
pub(crate) struct Envelope {
    /// Envelope type byte (0, 1, or 2).
    pub(crate) r#type: u8,
    /// Sender's X25519 public key (present only in type-1 envelopes).
    pub(crate) sender_public_key: Option<[u8; 32]>,
    /// 12-byte nonce.
    pub(crate) iv: [u8; 12],
    /// Ciphertext (or plaintext for type 2).
    pub(crate) sealed: Vec<u8>,
}

/// Serializes an envelope into the on-wire byte layout:
/// - Type 0: `[type ‖ iv ‖ sealed]`
/// - Type 1: `[type ‖ senderPubKey ‖ iv ‖ sealed]`
/// - Type 2: `[type ‖ sealed]`
pub(crate) fn serialize_envelope(
    r#type: u8,
    iv: &[u8; 12],
    sender: Option<&[u8; 32]>,
    sealed: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 12 + sealed.len());
    out.push(r#type);
    if r#type == ENVELOPE_TYPE_1 &&
        let Some(key) = sender
    {
        out.extend_from_slice(key);
    }
    if r#type != ENVELOPE_TYPE_2 {
        out.extend_from_slice(iv);
    }
    out.extend_from_slice(sealed);
    out
}

/// Parses an on-wire envelope byte buffer.
///
/// Returns `Err` for malformed input (too short, unknown type byte).
pub(crate) fn deserialize_envelope(bytes: &[u8]) -> Result<Envelope, WalletError> {
    if bytes.is_empty() {
        return Err(WalletError::Transport("envelope is empty".to_string()));
    }
    let r#type = bytes[0];
    match r#type {
        ENVELOPE_TYPE_0 => {
            if bytes.len() < 1 + IV_LENGTH {
                return Err(WalletError::Transport("type-0 envelope too short".to_string()));
            }
            let mut iv = [0u8; IV_LENGTH];
            iv.copy_from_slice(&bytes[1..=IV_LENGTH]);
            Ok(Envelope {
                r#type,
                sender_public_key: None,
                iv,
                sealed: bytes[1 + IV_LENGTH..].to_vec(),
            })
        }
        ENVELOPE_TYPE_1 => {
            if bytes.len() < 1 + KEY_LENGTH + IV_LENGTH {
                return Err(WalletError::Transport("type-1 envelope too short".to_string()));
            }
            let mut sender = [0u8; KEY_LENGTH];
            sender.copy_from_slice(&bytes[1..=KEY_LENGTH]);
            let mut iv = [0u8; IV_LENGTH];
            iv.copy_from_slice(&bytes[1 + KEY_LENGTH..1 + KEY_LENGTH + IV_LENGTH]);
            Ok(Envelope {
                r#type,
                sender_public_key: Some(sender),
                iv,
                sealed: bytes[1 + KEY_LENGTH + IV_LENGTH..].to_vec(),
            })
        }
        ENVELOPE_TYPE_2 => Ok(Envelope {
            r#type,
            sender_public_key: None,
            // Type 2 does not carry an IV; zero-fill (never used for type 2).
            iv: [0u8; IV_LENGTH],
            sealed: bytes[1..].to_vec(),
        }),
        other => Err(WalletError::Transport(format!("unknown envelope type byte: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn keypair_agreement_round_trips() {
        let a = WcKeyPair::generate();
        let b = WcKeyPair::generate();
        let s1 = a.shared_secret(&b.public_key());
        let s2 = b.shared_secret(&a.public_key());
        assert_eq!(s1.as_bytes(), s2.as_bytes());
    }

    #[test]
    fn derive_sym_key_is_deterministic_and_distinct() {
        let a = WcKeyPair::generate();
        let b = WcKeyPair::generate();
        let s = a.shared_secret(&b.public_key());
        let k1 = derive_sym_key(&s).expect("derive");
        let k2 = derive_sym_key(&s).expect("derive");
        assert_eq!(k1.as_bytes(), k2.as_bytes());
        let c = WcKeyPair::generate();
        let s2 = a.shared_secret(&c.public_key());
        let k3 = derive_sym_key(&s2).expect("derive");
        assert_ne!(k1.as_bytes(), k3.as_bytes());
    }

    #[test]
    fn envelope_type0_round_trip() {
        let key = WcSymKey::from_random();
        let msg = b"hello world";
        let sealed = WcCipher::seal_type0(&key, msg).expect("seal");
        assert_eq!(sealed[0], ENVELOPE_TYPE_0);
        assert_eq!(sealed.len(), 1 + IV_LENGTH + msg.len() + 16);
        let opened = WcCipher::open_type0(&key, &sealed).expect("open");
        assert_eq!(opened, msg);
        let other = WcSymKey::from_random();
        assert!(WcCipher::open_type0(&other, &sealed).is_err());
    }

    #[test]
    fn envelope_type1_round_trip_carries_sender_key() {
        let key = WcSymKey::from_random();
        let sender = [0xABu8; 32];
        let msg = b"authenticated";
        let sealed = WcCipher::seal_type1(&key, &sender, msg).expect("seal");
        assert_eq!(sealed[0], ENVELOPE_TYPE_1);
        assert_eq!(sealed.len(), 1 + KEY_LENGTH + IV_LENGTH + msg.len() + 16);
        let (recovered_sender, opened) = WcCipher::open_type1(&key, &sealed).expect("open");
        assert_eq!(recovered_sender, sender);
        assert_eq!(opened, msg);
    }

    #[test]
    fn type2_envelope_is_plaintext() {
        let bytes = serialize_envelope(ENVELOPE_TYPE_2, &[0u8; 12], None, b"raw");
        assert_eq!(bytes, [ENVELOPE_TYPE_2, b'r', b'a', b'w']);
        let env = deserialize_envelope(&bytes).expect("parse");
        assert_eq!(env.r#type, ENVELOPE_TYPE_2);
        assert_eq!(env.sealed, b"raw");
    }

    #[test]
    fn hash_bytes_matches_sha256_hex() {
        assert_eq!(hash_bytes(&[0x11; 32]), {
            let mut hasher = Sha256::new();
            hasher.update([0x11; 32]);
            hex::encode(hasher.finalize())
        });
    }
}
