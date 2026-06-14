use thiserror::Error;

/// Agent Circle error type with unique error codes for diagnostics.
///
/// Each variant carries a 4-digit code (E0xxx) printed in Display output.
/// Run `agent-circle doctor` to surface errors with codes.
#[derive(Error, Debug)]
pub enum AcError {
    /// E0001 — File, directory, or stream I/O failure.
    #[error("E0001: IO error — {0}")]
    Io(#[from] std::io::Error),

    /// E0002 — Identity key missing, malformed, or DID verification failed.
    #[error("E0002: Identity error — {0}")]
    Identity(String),

    /// E0003 — JSON or other serde serialization/deserialization failure.
    #[error("E0003: Serialization error — {0}")]
    Serialization(#[from] serde_json::Error),

    /// E0004 — Cryptographic key derivation, import, or signing failure.
    #[error("E0004: Key error — {0}")]
    Key(String),

    /// E0005 — Network / P2P transport failure (dial, listen, swarm).
    #[error("E0005: Network error — {0}")]
    Network(String),
}

impl AcError {
    /// Return the numeric error code for this variant.
    pub fn code(&self) -> &'static str {
        match self {
            AcError::Io(_) => "E0001",
            AcError::Identity(_) => "E0002",
            AcError::Serialization(_) => "E0003",
            AcError::Key(_) => "E0004",
            AcError::Network(_) => "E0005",
        }
    }

    /// Human-readable description of this error code.
    pub fn code_description(code: &str) -> &'static str {
        match code {
            "E0001" => "IO error — file, directory, or stream access failure",
            "E0002" => "Identity error — key missing, malformed, or DID verification failed",
            "E0003" => "Serialization error — JSON or serde encode/decode failure",
            "E0004" => "Key error — cryptographic key derivation, import, or signing failure",
            "E0005" => "Network error — P2P transport, dial, listen, or swarm failure",
            _ => "Unknown error code",
        }
    }
}

pub type AcResult<T> = std::result::Result<T, AcError>;
