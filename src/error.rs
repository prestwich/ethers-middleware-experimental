use crate::types::JsonRpcError;
use futures_channel::oneshot;
use thiserror::Error;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;

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

    /// A WebSocket transport error
    #[error("{0}")]
    WsError(#[from] WsError),

    /// Custom
    #[error("{0}")]
    CustomError(String),
}

#[derive(Error, Debug)]
/// Error thrown when sending a WS message
pub enum WsError {
    /// Thrown if the websocket responds with binary data
    #[error("Websocket responded with unexpected binary data")]
    UnexpectedBinary(Vec<u8>),

    /// Thrown if there's an error over the WS connection
    #[error(transparent)]
    #[cfg(not(target_arch = "wasm32"))]
    InternalWsError(#[from] tungstenite::Error),

    /// Thrown if there's an error over the WS connection
    #[error(transparent)]
    #[cfg(target_arch = "wasm32")]
    InternalWsError(#[from] ws_stream_wasm::WsErr),

    /// Channel Error
    #[error("{0}")]
    ChannelError(String),

    /// Oneshot cancelled
    #[error("{0}")]
    Canceled(#[from] oneshot::Canceled),

    /// Remote server sent a Close message
    #[error("Websocket closed with info: {0:?}")]
    #[cfg(not(target_arch = "wasm32"))]
    WsClosed(CloseFrame<'static>),

    /// Remote server sent a Close message
    #[error("Websocket closed with info")]
    #[cfg(target_arch = "wasm32")]
    WsClosed,

    /// Something caused the websocket to close
    #[error("WebSocket connection closed unexpectedly")]
    UnexpectedClose,
}

impl From<JsonRpcError> for RpcError {
    fn from(e: JsonRpcError) -> Self {
        RpcError::ErrorResponse(e)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<tungstenite::Error> for RpcError {
    fn from(e: tungstenite::Error) -> Self {
        RpcError::WsError(WsError::InternalWsError(e))
    }
}

#[cfg(target_arch = "wasm32")]
impl From<ws_stream_wasm::WsErr> for RpcError {
    fn from(e: ws_stream_wasm::WsErr) -> Self {
        RpcError::WsError(WsError::InternalWsError(e))
    }
}

impl From<oneshot::Canceled> for RpcError {
    fn from(e: oneshot::Canceled) -> Self {
        RpcError::WsError(WsError::Canceled(e))
    }
}
