use ethers_core::types::U256;
use futures_channel::{mpsc, oneshot};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{
    convert::Infallible,
    fmt::{self, Debug},
    str::FromStr,
};

use crate::{connections::RpcConnection, error::RpcError};

fn null_params(value: &Value) -> bool {
    matches!(value, Value::Null)
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct RawRequest {
    method: &'static str,
    #[serde(skip_serializing_if = "null_params")]
    params: Value,
}

impl RawRequest {
    /// Instantate a new request from method and params
    pub fn new<S>(method: &'static str, params: S) -> Result<Self, serde_json::Error>
    where
        S: Serialize,
    {
        Ok(RawRequest {
            method,
            params: serde_json::to_value(params)?,
        })
    }

    /// Access the request method
    pub fn method(&self) -> &str {
        self.method
    }

    /// Read the request params
    pub fn params(&self) -> &Value {
        &self.params
    }

    /// Get a mutable borrow of the params
    pub fn mut_params(&mut self) -> &mut Value {
        &mut self.params
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Deserialize, Debug, Clone, PartialEq)]
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
    #[allow(dead_code)]
    jsonrpc: String,
    #[serde(flatten)]
    pub result: RawResponse,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
/// A JSON-RPC Notifcation
pub struct JsonRpcNotification {
    jsonrpc: String,
    method: String,
    pub params: Notification,
}

#[derive(Deserialize, Debug, Clone, Eq)]
pub struct Notification {
    pub subscription: U256,
    pub result: Value,
}

impl PartialEq<Notification> for Notification {
    fn eq(&self, other: &Notification) -> bool {
        self.subscription == other.subscription && self.result == other.result
    }
}

// for convience in quorum provider
impl std::hash::Hash for Notification {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.subscription.hash(state);
        serde_json::to_string(&self.result).unwrap().hash(state);
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait RequestParams: Serialize + Send + Sync + Debug {
    const METHOD: &'static str;
    type Response: DeserializeOwned + std::fmt::Debug;

    fn to_raw_request(&self) -> RawRequest {
        RawRequest::new(Self::METHOD, self).expect("value ser doesn't fail")
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
    pub starting_block: u64,
    pub current_block: u64,
    pub highest_block: u64,
    pub known_states: u64,
    pub pulled_states: u64,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum SyncData {
    RawSyncing(bool),
    RawStatus(SyncDetails),
    TaggedSyncing { syncing: bool },
    TaggedStatus { status: SyncDetails },
}

pub(crate) type ResponseChannel = oneshot::Sender<Result<RawResponse, JsonRpcError>>;
pub(crate) type Subscription = mpsc::UnboundedSender<Notification>;

/// Instructions for the `WsServer` and `IpcServer`.
#[derive(Debug)]
pub(crate) enum Instruction {
    /// JSON-RPC request
    Request {
        request: JsonRpcRequest,
        sender: ResponseChannel,
    },
    /// Create a new subscription
    Subscribe { id: U256, sink: Subscription },
    /// Cancel an existing subscription
    Unsubscribe { id: U256 },
}

/// Node clients with potential client-specific behavior
#[derive(Clone, Debug)]
pub enum NodeClient {
    Geth,
    Erigon,
    OpenEthereum,
    Nethermind,
    Besu,
    Unknown(String),
}

impl NodeClient {
    pub fn parity_block_receipts(&self) -> bool {
        matches!(self, NodeClient::OpenEthereum | NodeClient::Nethermind)
    }

    pub fn supports_trace(&self) -> bool {
        matches!(self, NodeClient::OpenEthereum | NodeClient::Nethermind)
    }

    pub fn geth_like(&self) -> bool {
        matches!(self, NodeClient::Geth | NodeClient::Erigon)
    }
}

impl FromStr for NodeClient {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split('/').next().map(|s| s.to_lowercase()) {
            Some(name) => match name.as_ref() {
                "geth" => Ok(NodeClient::Geth),
                "erigon" => Ok(NodeClient::Erigon),
                "openethereum" => Ok(NodeClient::OpenEthereum),
                "nethermind" => Ok(NodeClient::Nethermind),
                "besu" => Ok(NodeClient::Besu),
                _ => Ok(NodeClient::Unknown(name)),
            },
            None => Ok(NodeClient::Unknown("".to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deser_response() {
        let response: JsonRpcResponse =
            serde_json::from_str(r#"{"jsonrpc": "2.0", "result": 19, "id": 1}"#).unwrap();
        assert_eq!(response.id, 1);
        assert_eq!(response.result.deserialize::<usize>().unwrap(), 19);
    }

    #[test]
    fn ser_request() {
        let req = JsonRpcRequest {
            id: 300,
            jsonrpc: "2.0",
            request: RawRequest {
                method: "method_name",
                params: serde_json::to_value(()).unwrap(),
            },
        };

        assert_eq!(
            &serde_json::to_string(&req).unwrap(),
            r#"{"id":300,"jsonrpc":"2.0","method":"method_name"}"#
        );

        let req = JsonRpcRequest {
            id: 300,
            jsonrpc: "2.0",
            request: RawRequest {
                method: "method_name",
                params: serde_json::to_value(1).unwrap(),
            },
        };
        assert_eq!(
            &serde_json::to_string(&req).unwrap(),
            r#"{"id":300,"jsonrpc":"2.0","method":"method_name","params":1}"#
        );
    }
}
