//! The stdio protocol between rBrowser and its helper `rbrowser-wsl`: one JSON object per line,
//! versioned, opened by a handshake.
//!
//! rBrowser writes `hello` with its [`PROTOCOL_VERSION`] and a fresh nonce; the helper answers
//! `welcome` with the same protocol, its own version and the nonce, or `error` with code
//! `protocol_mismatch` and the protocol it speaks. Both sides then name the mismatch the same
//! way ([`HandshakeError::ProtocolMismatch`]). The nonce ties the answer to this start, so a line
//! left over from an earlier helper is never taken for it. After the handshake: `ping`/`pong`,
//! `shutdown`, and the messages of the subcommands (RB-030-05, RB-030-06).

use serde::{Deserialize, Serialize};

use crate::watch::{Change, WatchRules};

/// The protocol both sides must speak; raised on every incompatible change.
pub const PROTOCOL_VERSION: u32 = 1;
/// The longest line either side reads; a longer one is refused, not buffered.
pub const MAX_LINE: usize = 64 * 1024;
/// A nonce: 16 random bytes as lowercase hex.
const NONCE_LEN: usize = 32;

/// One line of the protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Message {
    /// rBrowser opens the handshake.
    Hello {
        /// The protocol rBrowser speaks.
        protocol: u32,
        /// Echoed by the helper.
        nonce: String,
    },
    /// The helper accepts it.
    Welcome {
        /// The protocol the helper speaks.
        protocol: u32,
        /// The helper's package version.
        helper_version: String,
        /// The nonce of the `hello`.
        nonce: String,
    },
    /// A refusal; after a handshake refusal the helper exits.
    Error {
        /// What went wrong.
        code: ErrorCode,
        /// For the log.
        message: String,
        /// With `protocol_mismatch`: the protocol the helper speaks.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        protocol: Option<u32>,
    },
    /// Are you there?
    Ping,
    /// Yes.
    Pong,
    /// rBrowser ends the helper.
    Shutdown,
    /// `watch`: rBrowser starts the watch with these rules, or replaces them (RB-030-05).
    Watch {
        /// What counts and how long to debounce.
        rules: WatchRules,
    },
    /// `watch`: the helper watches now.
    Watching {
        /// The folders watched.
        folders: u64,
    },
    /// `watch`: paths that changed, debounced into one batch.
    Changes {
        /// Each path once, relative to the watched folder.
        changes: Vec<Change>,
    },
    /// `watch`: something changed, unknown what (events lost, too many changes).
    Rescan,
}

impl Message {
    /// The `type` of the message, for errors and logs.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::Welcome { .. } => "welcome",
            Self::Error { .. } => "error",
            Self::Ping => "ping",
            Self::Pong => "pong",
            Self::Shutdown => "shutdown",
            Self::Watch { .. } => "watch",
            Self::Watching { .. } => "watching",
            Self::Changes { .. } => "changes",
            Self::Rescan => "rescan",
        }
    }
}

/// The codes of an `error` message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The two sides speak different protocols.
    ProtocolMismatch,
    /// A line that is no message, or not one expected now.
    BadRequest,
    /// `watch`: a rule that does not compile.
    BadRule,
    /// `watch`: the folder cannot be watched (missing, too large, the system's watch limit).
    WatchFailed,
}

/// Why a handshake failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandshakeError {
    /// rBrowser and the helper speak different protocols: the helper is from another package.
    #[error(
        "protocol mismatch: rBrowser speaks protocol {app}, the WSL helper protocol {helper}; \
         both must come from the same rBrowser package"
    )]
    ProtocolMismatch {
        /// The protocol of rBrowser.
        app: u32,
        /// The protocol of the helper.
        helper: u32,
    },
    /// The answer carries another nonce.
    #[error("the WSL helper answered another handshake")]
    NonceMismatch,
    /// A message of another type than the handshake expects.
    #[error("unexpected message {0:?} in the handshake")]
    Unexpected(String),
    /// The other side refused the handshake.
    #[error("the handshake was refused: {0}")]
    Refused(String),
    /// A line that is no message.
    #[error("not a protocol message: {0}")]
    Malformed(String),
    /// A line longer than [`MAX_LINE`].
    #[error("a protocol line is longer than {MAX_LINE} bytes")]
    TooLong,
    /// The other side closed its output before the handshake ended.
    #[error("the WSL helper ended before the handshake")]
    Closed,
    /// No answer in time.
    #[error("the WSL helper did not answer the handshake in time")]
    Timeout,
}

impl HandshakeError {
    /// The i18n key of the message a user reads.
    #[must_use]
    pub fn i18n_key(&self) -> &'static str {
        match self {
            Self::ProtocolMismatch { .. } => "wsl.error.protocolMismatch",
            _ => "wsl.error.handshake",
        }
    }
}

/// `message` as one line, `\n` included.
#[must_use]
pub fn encode(message: &Message) -> String {
    let mut line = serde_json::to_string(message)
        .expect("a message is strings and numbers, which always serialise");
    line.push('\n');
    line
}

/// The message of one line (without or with its line end).
///
/// # Errors
///
/// [`HandshakeError::TooLong`] past [`MAX_LINE`], [`HandshakeError::Malformed`] for anything
/// that is no message.
pub fn decode(line: &str) -> Result<Message, HandshakeError> {
    if line.len() > MAX_LINE {
        return Err(HandshakeError::TooLong);
    }
    serde_json::from_str(line.trim_end())
        .map_err(|error| HandshakeError::Malformed(error.to_string()))
}

/// A fresh nonce for a `hello`.
#[must_use]
pub fn new_nonce() -> String {
    use rand::Rng as _;
    let mut bytes = [0u8; NONCE_LEN / 2];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Whether `nonce` has the form [`new_nonce`] writes.
#[must_use]
pub fn valid_nonce(nonce: &str) -> bool {
    nonce.len() == NONCE_LEN
        && nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// rBrowser's `hello` with `nonce`.
#[must_use]
pub fn hello(nonce: &str) -> Message {
    Message::Hello {
        protocol: PROTOCOL_VERSION,
        nonce: nonce.to_owned(),
    }
}

/// The helper's side: the reply to the first line, and whether the handshake succeeded (the
/// helper writes the reply either way, then exits on an error).
pub fn answer_hello(line: &str, helper_version: &str) -> (Message, Result<(), HandshakeError>) {
    let refuse = |error: HandshakeError| {
        let reply = Message::Error {
            code: ErrorCode::BadRequest,
            message: error.to_string(),
            protocol: None,
        };
        (reply, Err(error))
    };
    let (protocol, nonce) = match decode(line) {
        Ok(Message::Hello { protocol, nonce }) => (protocol, nonce),
        Ok(other) => return refuse(HandshakeError::Unexpected(other.kind().to_owned())),
        Err(error) => return refuse(error),
    };
    if !valid_nonce(&nonce) {
        return refuse(HandshakeError::Malformed(
            "the nonce is not 32 hex digits".into(),
        ));
    }
    if protocol != PROTOCOL_VERSION {
        let error = HandshakeError::ProtocolMismatch {
            app: protocol,
            helper: PROTOCOL_VERSION,
        };
        let reply = Message::Error {
            code: ErrorCode::ProtocolMismatch,
            message: error.to_string(),
            protocol: Some(PROTOCOL_VERSION),
        };
        return (reply, Err(error));
    }
    let welcome = Message::Welcome {
        protocol: PROTOCOL_VERSION,
        helper_version: helper_version.to_owned(),
        nonce,
    };
    (welcome, Ok(()))
}

/// rBrowser's side: the helper's version from its answer to the `hello` with `nonce`.
///
/// # Errors
///
/// [`HandshakeError::ProtocolMismatch`] when the helper speaks another protocol (refused or
/// welcomed), [`HandshakeError::NonceMismatch`], [`HandshakeError::Refused`] or
/// [`HandshakeError::Unexpected`].
pub fn check_welcome(answer: &Message, nonce: &str) -> Result<String, HandshakeError> {
    match answer {
        Message::Welcome {
            protocol,
            helper_version,
            nonce: echoed,
        } => {
            if *protocol != PROTOCOL_VERSION {
                return Err(HandshakeError::ProtocolMismatch {
                    app: PROTOCOL_VERSION,
                    helper: *protocol,
                });
            }
            if echoed != nonce {
                return Err(HandshakeError::NonceMismatch);
            }
            Ok(helper_version.clone())
        }
        Message::Error {
            code: ErrorCode::ProtocolMismatch,
            protocol: Some(helper),
            ..
        } => Err(HandshakeError::ProtocolMismatch {
            app: PROTOCOL_VERSION,
            helper: *helper,
        }),
        Message::Error { message, .. } => Err(HandshakeError::Refused(message.clone())),
        other => Err(HandshakeError::Unexpected(other.kind().to_owned())),
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
