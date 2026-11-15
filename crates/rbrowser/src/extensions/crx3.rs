//! The CRX3 container of a Web Store extension (RB-080-23), checked as Chromium's
//! `components/crx_file/crx_verifier.cc` checks it, before a byte of it is unpacked:
//!
//! - `Cr24`, version 3, the header's length (at most [`MAX_HEADER_BYTES`]), the header, the ZIP.
//! - The header is a `CrxFileHeader` protobuf ([`proto`]): `sha256_with_rsa` (2) and
//!   `sha256_with_ecdsa` (3) proofs, each `{ public_key = 1, signature = 2 }`, and
//!   `signed_header_data` (10000), a `SignedData { crx_id = 1 }` of 16 bytes.
//! - Every proof signs `"CRX3 SignedData\0"` + the little-endian `u32` length of
//!   `signed_header_data` + `signed_header_data` + the ZIP; every signature must verify.
//! - The **developer proof**: a key whose id (the first 16 bytes of the SHA-256 of its DER,
//!   written `a` to `p`) is the declared `crx_id`, which must be the id the person asked for.
//! - The **publisher proof**: a key whose SHA-256 is the Web Store's
//!   ([`WEB_STORE_PUBLISHER_KEY_HASH`]); the tests use their own.
//!
//! No token, path or key reaches a log or an error: the errors name the id at most.

pub mod proto;
pub mod spki;

use ring::signature::{ECDSA_P256_SHA256_ASN1, RSA_PKCS1_2048_8192_SHA256, UnparsedPublicKey};
use sha2::{Digest as _, Sha256};

use self::spki::KeyKind;
use super::manifest::id_from_der;

/// The file's magic.
pub const MAGIC: &[u8; 4] = b"Cr24";
/// The format version.
pub const VERSION: u32 = 3;
/// The largest header read: a real one holds a few keys and signatures.
pub const MAX_HEADER_BYTES: u32 = 1024 * 1024;
/// The SHA-256 of the DER SPKI of the Chrome Web Store's publisher key (`ecdsa_2017_public`):
/// `kPublisherKeyHash` in Chromium's `components/crx_file/crx_verifier.cc`.
pub const WEB_STORE_PUBLISHER_KEY_HASH: [u8; 32] = [
    0x61, 0xf7, 0xf2, 0xa6, 0xbf, 0xcf, 0x74, 0xcd, 0x0b, 0xc1, 0xfe, 0x24, 0x97, 0xcc, 0x9b, 0x04,
    0x25, 0x4c, 0x65, 0x8f, 0x79, 0xf2, 0x14, 0x53, 0x92, 0x86, 0x7e, 0xa8, 0x36, 0x63, 0x67, 0xcf,
];
/// What every proof signs first.
const SIGNATURE_CONTEXT: &[u8] = b"CRX3 SignedData\0";
const FIELD_RSA: u64 = 2;
const FIELD_ECDSA: u64 = 3;
const FIELD_SIGNED_HEADER: u64 = 10_000;
const FIELD_PUBLIC_KEY: u64 = 1;
const FIELD_SIGNATURE: u64 = 2;
const FIELD_CRX_ID: u64 = 1;
const CRX_ID_BYTES: usize = 16;

/// A CRX that passed every check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verified<'a> {
    /// The extension id, the developer key's.
    pub id: String,
    /// The developer's public key, DER SPKI (the manifest's `key`, base64).
    pub public_key: Vec<u8>,
    /// The ZIP payload.
    pub zip: &'a [u8],
}

/// Why a CRX is refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Crx3Error {
    /// Not `Cr24`.
    #[error("the file is not a CRX")]
    BadMagic,
    /// Not version 3.
    #[error("the file is CRX version {0}; rBrowser installs CRX3 only")]
    BadVersion(u32),
    /// A header larger than any real one.
    #[error("the CRX header is larger than 1 MiB")]
    HeaderTooLarge,
    /// The file ends before the header or the ZIP.
    #[error("the file is truncated")]
    Truncated,
    /// The header is no `CrxFileHeader`.
    #[error("the CRX header cannot be read: {0}")]
    BadHeader(String),
    /// No proof's key is the declared id's.
    #[error("the CRX carries no proof of the extension's developer")]
    NoDeveloperProof,
    /// A proof's key is neither RSA nor P-256.
    #[error("the CRX is signed with a key rBrowser cannot check")]
    UnsupportedKey,
    /// The file is another extension's.
    #[error("the file is the extension {found}, not {requested}")]
    IdMismatch {
        /// The id asked for.
        requested: String,
        /// The id the file declares.
        found: String,
    },
    /// A signature does not match the contents.
    #[error("a signature of the CRX does not match its contents")]
    BadSignature,
    /// No proof by the Web Store.
    #[error("the CRX carries no proof of the Chrome Web Store")]
    NoPublisherProof,
}

impl Crx3Error {
    /// The stable code of the error (MCP answers, notices).
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadMagic => "extensions.crx_not_crx",
            Self::BadVersion(_) => "extensions.crx_version",
            Self::HeaderTooLarge => "extensions.crx_header_too_large",
            Self::Truncated => "extensions.crx_truncated",
            Self::BadHeader(_) => "extensions.crx_header_invalid",
            Self::NoDeveloperProof => "extensions.crx_no_developer_proof",
            Self::UnsupportedKey => "extensions.crx_unsupported_key",
            Self::IdMismatch { .. } => "extensions.crx_id_mismatch",
            Self::BadSignature => "extensions.crx_bad_signature",
            Self::NoPublisherProof => "extensions.crx_no_publisher_proof",
        }
    }
}

/// One proof of the header.
struct Proof<'a> {
    public_key: &'a [u8],
    signature: &'a [u8],
    kind: KeyKind,
}

/// Checks `bytes` as the CRX of the extension `expected_id`, signed by the publisher whose key
/// hashes to `publisher_key_hash`.
///
/// # Errors
///
/// The first check that fails; nothing of the file is trusted then.
pub fn verify<'a>(
    bytes: &'a [u8],
    expected_id: &str,
    publisher_key_hash: &[u8; 32],
) -> Result<Verified<'a>, Crx3Error> {
    let (header, zip) = split(bytes)?;
    let (proofs, signed_header) = parse_header(header)?;
    let declared = crx_id(signed_header)?;
    // The signed bytes, built once for every proof.
    let mut signed =
        Vec::with_capacity(SIGNATURE_CONTEXT.len() + 4 + signed_header.len() + zip.len());
    signed.extend_from_slice(SIGNATURE_CONTEXT);
    let length = u32::try_from(signed_header.len()).map_err(|_| Crx3Error::HeaderTooLarge)?;
    signed.extend_from_slice(&length.to_le_bytes());
    signed.extend_from_slice(signed_header);
    signed.extend_from_slice(zip);

    let mut developer: Option<&[u8]> = None;
    let mut publisher = false;
    for proof in &proofs {
        check_signature(proof, &signed)?;
        if id_from_der(proof.public_key) == declared {
            developer = Some(proof.public_key);
        }
        if Sha256::digest(proof.public_key).as_slice() == publisher_key_hash {
            publisher = true;
        }
    }
    let public_key = developer.ok_or(Crx3Error::NoDeveloperProof)?;
    if declared != expected_id {
        return Err(Crx3Error::IdMismatch {
            requested: expected_id.to_owned(),
            found: declared,
        });
    }
    if !publisher {
        return Err(Crx3Error::NoPublisherProof);
    }
    Ok(Verified {
        id: declared,
        public_key: public_key.to_vec(),
        zip,
    })
}

/// The header and the ZIP of `bytes`.
fn split(bytes: &[u8]) -> Result<(&[u8], &[u8]), Crx3Error> {
    let Some(magic) = bytes.get(..MAGIC.len()) else {
        return Err(if bytes.is_empty() || MAGIC.starts_with(bytes) {
            Crx3Error::Truncated
        } else {
            Crx3Error::BadMagic
        });
    };
    if magic != MAGIC {
        return Err(Crx3Error::BadMagic);
    }
    let version = word(bytes, 4)?;
    if version != VERSION {
        return Err(Crx3Error::BadVersion(version));
    }
    let header_len = word(bytes, 8)?;
    if header_len > MAX_HEADER_BYTES {
        return Err(Crx3Error::HeaderTooLarge);
    }
    let start = 12;
    let end = start + usize::try_from(header_len).map_err(|_| Crx3Error::HeaderTooLarge)?;
    let header = bytes.get(start..end).ok_or(Crx3Error::Truncated)?;
    let zip = bytes.get(end..).ok_or(Crx3Error::Truncated)?;
    if zip.is_empty() {
        return Err(Crx3Error::Truncated);
    }
    Ok((header, zip))
}

/// The little-endian `u32` at `at`.
fn word(bytes: &[u8], at: usize) -> Result<u32, Crx3Error> {
    let slice = bytes.get(at..at + 4).ok_or(Crx3Error::Truncated)?;
    let array: [u8; 4] = slice.try_into().map_err(|_| Crx3Error::Truncated)?;
    Ok(u32::from_le_bytes(array))
}

/// The proofs and the `signed_header_data` of `header`.
fn parse_header(header: &[u8]) -> Result<(Vec<Proof<'_>>, &[u8]), Crx3Error> {
    let bad = |error: proto::ProtoError| Crx3Error::BadHeader(error.to_string());
    let mut proofs = Vec::new();
    let mut signed_header = None;
    for field in proto::fields(header).map_err(bad)? {
        let kind = match field.number {
            FIELD_RSA => KeyKind::Rsa,
            FIELD_ECDSA => KeyKind::EcP256,
            FIELD_SIGNED_HEADER => {
                signed_header = Some(field.bytes);
                continue;
            }
            _ => continue,
        };
        let mut public_key = None;
        let mut signature = None;
        for part in proto::fields(field.bytes).map_err(bad)? {
            match part.number {
                FIELD_PUBLIC_KEY => public_key = Some(part.bytes),
                FIELD_SIGNATURE => signature = Some(part.bytes),
                _ => {}
            }
        }
        let (Some(public_key), Some(signature)) = (public_key, signature) else {
            return Err(Crx3Error::BadHeader(
                "a proof lacks its key or signature".to_owned(),
            ));
        };
        proofs.push(Proof {
            public_key,
            signature,
            kind,
        });
    }
    let signed_header =
        signed_header.ok_or_else(|| Crx3Error::BadHeader("no signed_header_data".to_owned()))?;
    Ok((proofs, signed_header))
}

/// The declared id of `signed_header` (`SignedData.crx_id`), written `a` to `p`.
fn crx_id(signed_header: &[u8]) -> Result<String, Crx3Error> {
    let bad = |error: proto::ProtoError| Crx3Error::BadHeader(error.to_string());
    let id = proto::fields(signed_header)
        .map_err(bad)?
        .into_iter()
        .find(|field| field.number == FIELD_CRX_ID)
        .map(|field| field.bytes)
        .ok_or_else(|| Crx3Error::BadHeader("no crx_id".to_owned()))?;
    if id.len() != CRX_ID_BYTES {
        return Err(Crx3Error::BadHeader(format!(
            "crx_id has {} bytes, not {CRX_ID_BYTES}",
            id.len()
        )));
    }
    Ok(id
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .map(|nibble| char::from(b'a' + nibble))
        .collect())
}

/// Checks `proof`'s signature over `message`.
fn check_signature(proof: &Proof<'_>, message: &[u8]) -> Result<(), Crx3Error> {
    let key = spki::parse(proof.public_key).ok_or(Crx3Error::UnsupportedKey)?;
    if key.kind != proof.kind {
        return Err(Crx3Error::UnsupportedKey);
    }
    let result = match key.kind {
        KeyKind::Rsa => UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, key.key)
            .verify(message, proof.signature),
        KeyKind::EcP256 => UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, key.key)
            .verify(message, proof.signature),
    };
    result.map_err(|_| Crx3Error::BadSignature)
}

#[cfg(test)]
#[path = "crx3_tests.rs"]
mod tests;
