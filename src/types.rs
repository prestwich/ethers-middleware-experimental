use async_trait::async_trait;
use ethers::prelude::U64;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::fmt::{self, Debug};

use crate::{error::RpcError, provider::RpcConnection};

fn null_params(value: &Value) -> bool {
    matches!(value, Value::Null)
}

#[derive(Serialize, Debug)]
pub struct RawRequest<'a> {
    method: &'a str,
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
pub struct JsonRpcRequest<'a> {
    pub(crate) id: u64,
    pub(crate) jsonrpc: &'static str,
    #[serde(flatten)]
    pub(crate) request: RawRequest<'a>,
}

#[derive(Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub(crate) id: u64,
    jsonrpc: String,
    #[serde(flatten)]
    pub result: RawResponse,
}

#[async_trait]
pub trait RequestParams: Serialize + Send + Sync + Debug {
    const METHOD: &'static str;
    type Response: DeserializeOwned + std::fmt::Debug;

    fn to_raw_request(&self) -> RawRequest<'static> {
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

#[derive(Serialize, Debug)]
pub struct GetBlockHeightParams;

impl RequestParams for GetBlockHeightParams {
    const METHOD: &'static str = "eth_blockNumber";

    type Response = U64;
}
