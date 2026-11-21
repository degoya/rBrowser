//! A viewer's WebSocket frames on their way to the dev server (RB-096-05): a viewer of the
//! presentation watches, it sends nothing. The dev server's socket (Vite's HMR, Next.js') reaches
//! the viewer as it comes, but of the viewer's frames only the control frames (close, ping, pong)
//! go on; every data frame - text, binary and their continuations, `vite:invalidate` or any
//! `import.meta.hot.send` among them - is dropped at the portal. A frame that breaks the
//! protocol (unmasked, a control frame over 125 bytes or fragmented) ends the upload.

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

/// The largest payload of a control frame (RFC 6455, 5.5).
const MAX_CONTROL_PAYLOAD: u64 = 125;

/// A viewer's frame broke the WebSocket protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Broken;

/// The filter over one connection's client-to-server bytes.
#[derive(Debug, Default)]
pub struct ControlOnly {
    /// Bytes of a frame whose head (or, for a control frame, whole frame) is not complete yet.
    pending: Vec<u8>,
    /// Bytes of a dropped data frame still to come.
    skip: u64,
}

/// A frame's head: its length, the payload's length, whether it is a control frame, FIN, MASK.
struct Head {
    len: usize,
    payload: u64,
    control: bool,
    fin: bool,
    masked: bool,
}

fn head_of(bytes: &[u8]) -> Option<Head> {
    let first = *bytes.first()?;
    let second = *bytes.get(1)?;
    let masked = second & 0x80 != 0;
    let (extended, payload) = match second & 0x7f {
        126 => (
            2,
            u64::from(u16::from_be_bytes([*bytes.get(2)?, *bytes.get(3)?])),
        ),
        127 => {
            let mut length = [0u8; 8];
            length.copy_from_slice(bytes.get(2..10)?);
            (8, u64::from_be_bytes(length))
        }
        short => (0, u64::from(short)),
    };
    let len = 2 + extended + if masked { 4 } else { 0 };
    (bytes.len() >= len).then_some(Head {
        len,
        payload,
        control: first & 0x08 != 0,
        fin: first & 0x80 != 0,
        masked,
    })
}

impl ControlOnly {
    /// Takes the next `input` of the viewer's side; the bytes to send on go to `out`.
    ///
    /// # Errors
    ///
    /// [`Broken`] for an unmasked frame or a control frame over 125 bytes or without FIN.
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) -> Result<(), Broken> {
        let mut input = input;
        if self.skip > 0 {
            let skipped = usize::try_from(self.skip)
                .unwrap_or(usize::MAX)
                .min(input.len());
            input = input.get(skipped..).unwrap_or_default();
            self.skip -= skipped as u64;
            if self.skip > 0 {
                return Ok(());
            }
        }
        self.pending.extend_from_slice(input);
        loop {
            let Some(head) = head_of(&self.pending) else {
                return Ok(());
            };
            if !head.masked {
                return Err(Broken);
            }
            let total = (head.len as u64).saturating_add(head.payload);
            let have = self.pending.len() as u64;
            if head.control {
                if head.payload > MAX_CONTROL_PAYLOAD || !head.fin {
                    return Err(Broken);
                }
                if have < total {
                    return Ok(());
                }
                // At most 14 + 125 bytes: `total` fits.
                let end = usize::try_from(total).map_err(|_| Broken)?;
                out.extend(self.pending.drain(..end));
            } else if have >= total {
                let end = usize::try_from(total).map_err(|_| Broken)?;
                self.pending.drain(..end);
            } else {
                self.skip = total - have;
                self.pending.clear();
                return Ok(());
            }
        }
    }
}

/// Copies the viewer's control frames from `from` to `to`, starting from `filter` (which may
/// have taken bytes already), until `from` ends or a frame breaks the protocol.
///
/// # Errors
///
/// A socket failed, or `InvalidData` for a broken frame.
pub async fn copy_control_frames<R, W>(
    mut filter: ControlOnly,
    from: &mut R,
    to: &mut W,
) -> std::io::Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut chunk = vec![0u8; 16 * 1024];
    let mut out = Vec::new();
    let mut sent = 0u64;
    loop {
        let read = from.read(&mut chunk).await?;
        if read == 0 {
            return Ok(sent);
        }
        out.clear();
        filter
            .feed(chunk.get(..read).unwrap_or_default(), &mut out)
            .map_err(|Broken| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "a viewer's frame broke the protocol",
                )
            })?;
        if !out.is_empty() {
            to.write_all(&out).await?;
            sent += out.len() as u64;
        }
    }
}

#[cfg(test)]
#[path = "frames_tests.rs"]
mod tests;
