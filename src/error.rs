use crate::types::JsonRpcError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    /// SerdeJson
    #[error("{0}")]
    SerdeJson(#[from] serde_json::Error),
    /// ErrorResponse
    #[error("{0}")]
    ErrorResponse(JsonRpcError),
    /// Reqwest
    #[error("{0}")]
    Reqwest(#[from] reqwest::Error),

    /// Attempted to sign a transaction without a signer
    #[error("Attempted to sign a transaction without a signer")]
    SignerUnavailable,

    /// Attempted to resolve ENS on a network with no known deployment
    #[error("No known ENS deployment for chain id {0}")]
    NoKnownEns(u64),

    /// An error during ENS name resolution
    #[error("ens name not found: {0}")]
    EnsError(String),

    /// Custom
    #[error("{0}")]
    CustomError(String),
}

impl From<JsonRpcError> for RpcError {
    fn from(e: JsonRpcError) -> Self {
        RpcError::ErrorResponse(e)
    }
}
