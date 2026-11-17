//! The public key of a CRX3 proof: a DER `SubjectPublicKeyInfo` (what the extension id is
//! hashed from), read down to the bytes `ring` verifies with - the `RSAPublicKey` of an RSA key,
//! the uncompressed point of a P-256 key. Two algorithms, nothing else; a key that is neither
//! is no key.

/// The algorithm of a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    /// `rsaEncryption`: `RSA_PKCS1_2048_8192_SHA256`.
    Rsa,
    /// `ecPublicKey` with `prime256v1`: `ECDSA_P256_SHA256_ASN1`.
    EcP256,
}

/// A key `ring` can verify with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicKey<'a> {
    /// The algorithm.
    pub kind: KeyKind,
    /// The bytes for `ring::signature::UnparsedPublicKey`.
    pub key: &'a [u8],
}

/// `rsaEncryption` (1.2.840.113549.1.1.1).
const RSA_OID: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
/// `ecPublicKey` (1.2.840.10045.2.1).
const EC_OID: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
/// `prime256v1` (1.2.840.10045.3.1.7).
const P256_OID: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
const SEQUENCE: u8 = 0x30;
const OBJECT_IDENTIFIER: u8 = 0x06;
const BIT_STRING: u8 = 0x03;

/// The key of the DER `SubjectPublicKeyInfo` `der`; `None` for anything else.
pub fn parse(der: &[u8]) -> Option<PublicKey<'_>> {
    let (info, rest) = element(der, SEQUENCE)?;
    if !rest.is_empty() {
        return None;
    }
    let (algorithm, after_algorithm) = element(info, SEQUENCE)?;
    let (oid, parameters) = element(algorithm, OBJECT_IDENTIFIER)?;
    let (bits, after_key) = element(after_algorithm, BIT_STRING)?;
    if !after_key.is_empty() {
        return None;
    }
    // The first byte of a BIT STRING counts the unused bits of its last byte: none here.
    let (unused, key) = bits.split_first()?;
    if *unused != 0 || key.is_empty() {
        return None;
    }
    let kind = if oid == RSA_OID {
        KeyKind::Rsa
    } else if oid == EC_OID {
        let (curve, rest) = element(parameters, OBJECT_IDENTIFIER)?;
        if curve != P256_OID || !rest.is_empty() {
            return None;
        }
        KeyKind::EcP256
    } else {
        return None;
    };
    Some(PublicKey { kind, key })
}

/// The contents of the DER element of `tag` at the start of `der`, and what follows it.
fn element(der: &[u8], tag: u8) -> Option<(&[u8], &[u8])> {
    let (first, rest) = der.split_first()?;
    if *first != tag {
        return None;
    }
    let (head, rest) = rest.split_first()?;
    let (length, rest) = if *head < 0x80 {
        (usize::from(*head), rest)
    } else {
        let count = usize::from(*head & 0x7f);
        if count == 0 || count > 4 || rest.len() < count {
            return None;
        }
        let (bytes, rest) = rest.split_at(count);
        let length = bytes
            .iter()
            .fold(0_usize, |length, byte| (length << 8) | usize::from(*byte));
        (length, rest)
    };
    (length <= rest.len()).then(|| rest.split_at(length))
}

#[cfg(test)]
#[path = "spki_tests.rs"]
mod tests;
