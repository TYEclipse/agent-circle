use thiserror::Error;

/// Agent Circle error type (named AcError to avoid conflicting with
/// libp2p's `#[derive(NetworkBehaviour)]` which resolves `Error` at crate
/// level and picks up any type literally named `Error`).
#[derive(Error, Debug)]
pub enum AcError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Identity error: {0}")]
    Identity(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Key error: {0}")]
    Key(String),

    #[error("Network error: {0}")]
    Network(String),
}

pub type AcResult<T> = std::result::Result<T, AcError>;
