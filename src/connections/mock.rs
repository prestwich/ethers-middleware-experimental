use crate::{
    connections::RpcConnection,
    error::RpcError,
    types::{RawRequest, RawResponse},
};

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use std::{
    borrow::Borrow,
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
/// Mock transport used in test environments.
pub struct MockRpcConnection {
    requests: Arc<Mutex<VecDeque<(String, Value)>>>,
    responses: Arc<Mutex<VecDeque<Value>>>,
}

impl Default for MockRpcConnection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl RpcConnection for MockRpcConnection {
    /// Pushes the `(method, input)` to the back of the `requests` queue,
    /// pops the responses from the back of the `responses` queue
    async fn _request(&self, request: RawRequest) -> Result<RawResponse, RpcError> {
        self.requests
            .lock()
            .unwrap()
            .push_back((request.method().to_owned(), request.params().clone()));
        let mut responses = self.responses.lock().unwrap();
        let response = responses
            .pop_back()
            .ok_or_else(|| RpcError::CustomError("empty responses".to_owned()))?;

        Ok(RawResponse::Success { result: response })
    }
}

impl MockRpcConnection {
    /// Checks that the provided request was submitted by the client
    pub fn assert_request<T: Serialize + Send + Sync>(
        &self,
        method: &str,
        data: T,
    ) -> Result<(), RpcError> {
        let (m, inp) = self
            .requests
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| RpcError::CustomError("empty requests".to_owned()))?;
        assert_eq!(m, method);
        assert_eq!(
            serde_json::to_value(data).expect("could not serialize data"),
            inp
        );
        Ok(())
    }

    /// Instantiates a mock transport
    pub fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(VecDeque::new())),
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Pushes the data to the responses
    pub fn push<T: Serialize + Send + Sync, K: Borrow<T>>(&self, data: K) -> Result<(), RpcError> {
        let value = serde_json::to_value(data.borrow())?;
        self.responses.lock().unwrap().push_back(value);
        Ok(())
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use crate::connections::RpcConnection;
    use ethers_core::types::U64;

    #[tokio::test]
    async fn pushes_request_and_response() {
        let mock = MockRpcConnection::new();
        mock.push(U64::from(12)).unwrap();
        let block: U64 = mock
            .call_method("eth_blockNumber", ())
            .await
            .unwrap()
            .deserialize()
            .unwrap();
        mock.assert_request("eth_blockNumber", ()).unwrap();
        assert_eq!(block.as_u64(), 12);
    }

    #[tokio::test]
    async fn empty_responses() {
        let mock = MockRpcConnection::new();
        // tries to get a response without pushing a response
        let err = mock.call_method("eth_blockNumber", ()).await.unwrap_err();
        match err {
            RpcError::CustomError(e) => {
                assert!(e.contains("empty responses"));
            }
            _ => panic!("expected empty responses"),
        };
    }

    #[tokio::test]
    async fn empty_requests() {
        let mock = MockRpcConnection::new();
        // tries to assert a request without making one
        let err = mock.assert_request("eth_blockNumber", ()).unwrap_err();
        match err {
            RpcError::CustomError(e) => {
                assert!(e.contains("empty requests"));
            }
            _ => panic!("expected empty requests"),
        };
    }
}
