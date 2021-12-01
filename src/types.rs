use async_trait::async_trait;
use ethers::prelude::U256;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::fmt::{self, Debug};

use crate::{connections::RpcConnection, error::RpcError};

fn null_params(value: &Value) -> bool {
    matches!(value, Value::Null)
}

#[derive(Serialize, Debug)]
pub struct RawRequest {
    method: &'static str,
    #[serde(skip_serializing_if = "null_params")]
    params: Value,
}

#[derive(Deserialize, Debug, Clone)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "(code: {}, message: {}, data: {:?})",
            self.code, self.message, self.data
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum RawResponse {
    Success { result: Value },
    Error { error: JsonRpcError },
}

impl RawResponse {
    pub fn is_error(&self) -> bool {
        use RawResponse::*;
        match &self {
            Success { result: _ } => false,
            Error { error: _ } => true,
        }
    }

    pub fn is_success(&self) -> bool {
        !self.is_error()
    }

    pub fn into_result(self) -> Result<Value, JsonRpcError> {
        match self {
            RawResponse::Success { result } => Ok(result),
            RawResponse::Error { error } => Err(error),
        }
    }

    pub fn deserialize<R>(self) -> Result<R, RpcError>
    where
        R: DeserializeOwned,
    {
        match self {
            RawResponse::Success { result } => Ok(serde_json::from_value(result)?),
            RawResponse::Error { error } => Err(RpcError::ErrorResponse(error)),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct JsonRpcRequest {
    pub(crate) id: u64,
    pub(crate) jsonrpc: &'static str,
    #[serde(flatten)]
    pub(crate) request: RawRequest,
}

#[derive(Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub(crate) id: u64,
    jsonrpc: String,
    #[serde(flatten)]
    pub result: RawResponse,
}

#[derive(Serialize, Deserialize, Debug)]
/// A JSON-RPC Notifcation
pub struct JsonRpcNotification {
    jsonrpc: String,
    method: String,
    pub params: Notification,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Notification {
    pub subscription: U256,
    pub result: Value,
}

#[async_trait]
pub trait RequestParams: Serialize + Send + Sync + Debug {
    const METHOD: &'static str;
    type Response: DeserializeOwned + std::fmt::Debug;

    fn to_raw_request(&self) -> RawRequest {
        RawRequest {
            method: Self::METHOD,
            params: serde_json::to_value(self).expect("value ser doesn't fail"),
        }
    }

    async fn send_via(&self, connection: &dyn RpcConnection) -> Result<Self::Response, RpcError> {
        connection
            ._request(self.to_raw_request())
            .await?
            .deserialize()
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SyncDetails {
    starting_block: u64,
    current_block: u64,
    highest_block: u64,
    known_states: u64,
    pulled_states: u64,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum SyncData {
    RawSyncing(bool),
    RawStatus(SyncDetails),
    TaggedSyncing { syncing: bool },
    TaggedStatus { status: SyncDetails },
}
