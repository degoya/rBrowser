//! A small protobuf reader for the CRX3 header (RB-080-23): the length-delimited fields of
//! one message, in order, with varint and fixed-width fields skipped. No protobuf dependency:
//! `CrxFileHeader`, `AsymmetricKeyProof` and `SignedData` hold `bytes` fields only.

/// A length-delimited field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field<'a> {
    /// The field number.
    pub number: u64,
    /// Its bytes.
    pub bytes: &'a [u8],
}

/// Why a message is refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProtoError {
    /// The message ends inside a field.
    #[error("the message ends inside a field")]
    Truncated,
    /// A varint longer than ten bytes.
    #[error("a varint is longer than ten bytes")]
    BadVarint,
    /// A group or an unknown wire type.
    #[error("the wire type {0} is not supported")]
    BadWireType(u8),
}

/// The most bytes of one varint.
const MAX_VARINT_BYTES: usize = 10;

/// Every length-delimited field of `buffer`, in order.
///
/// # Errors
///
/// A truncated or malformed message.
pub fn fields(buffer: &[u8]) -> Result<Vec<Field<'_>>, ProtoError> {
    let mut fields = Vec::new();
    let mut at = 0;
    while at < buffer.len() {
        let (tag, next) = varint(buffer, at)?;
        at = next;
        let number = tag >> 3;
        let wire = u8::try_from(tag & 7).unwrap_or(u8::MAX);
        match wire {
            0 => at = varint(buffer, at)?.1,
            1 => at = skip(buffer, at, 8)?,
            2 => {
                let (length, start) = varint(buffer, at)?;
                let length = usize::try_from(length).map_err(|_| ProtoError::Truncated)?;
                let end = skip(buffer, start, length)?;
                fields.push(Field {
                    number,
                    bytes: buffer.get(start..end).ok_or(ProtoError::Truncated)?,
                });
                at = end;
            }
            5 => at = skip(buffer, at, 4)?,
            other => return Err(ProtoError::BadWireType(other)),
        }
    }
    Ok(fields)
}

/// The varint at `at` and the position after it.
fn varint(buffer: &[u8], at: usize) -> Result<(u64, usize), ProtoError> {
    let mut value: u64 = 0;
    for (index, byte) in buffer
        .get(at..)
        .ok_or(ProtoError::Truncated)?
        .iter()
        .take(MAX_VARINT_BYTES)
        .enumerate()
    {
        value |= u64::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Ok((value, at + index + 1));
        }
    }
    if buffer.len().saturating_sub(at) >= MAX_VARINT_BYTES {
        Err(ProtoError::BadVarint)
    } else {
        Err(ProtoError::Truncated)
    }
}

/// The position `count` bytes after `at`, when `buffer` holds them.
fn skip(buffer: &[u8], at: usize, count: usize) -> Result<usize, ProtoError> {
    let end = at.checked_add(count).ok_or(ProtoError::Truncated)?;
    (end <= buffer.len())
        .then_some(end)
        .ok_or(ProtoError::Truncated)
}

#[cfg(test)]
#[path = "proto_tests.rs"]
mod tests;
