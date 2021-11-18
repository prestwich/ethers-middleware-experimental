use std::{
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

use async_trait::async_trait;
use reqwest::{Client, Url};

use crate::{
    error::RpcError,
    provider::RpcConnection,
    types::{JsonRpcRequest, JsonRpcResponse, RawRequest, RawResponse},
};

/// A low-level JSON-RPC Client over HTTP.
///
/// # Example
///
/// ```no_run
/// use ethers_core::types::U64;
/// use ethers_providers::{JsonRpcClient, Http};
/// use std::str::FromStr;
///
/// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = Http::from_str("http://localhost:8545")?;
/// let block_number: U64 = provider.request("eth_blockNumber", ()).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Http {
    id: AtomicU64,
    client: Client,
    url: Url,
}

impl Http {
    /// Initializes a new HTTP Client
    ///
    /// # Example
    ///
    /// ```
    /// use ethers_providers::Http;
    /// use url::Url;
    ///
    /// let url = Url::parse("http://localhost:8545").unwrap();
    /// let provider = Http::new(url);
    /// ```
    pub fn new(url: impl Into<Url>) -> Self {
        Self {
            id: AtomicU64::new(0),
            client: Client::new(),
            url: url.into(),
        }
    }
}

#[async_trait]
impl RpcConnection for Http {
    async fn _request(&self, request: RawRequest<'_>) -> Result<RawResponse, RpcError> {
        let id = self.id.fetch_add(1, Ordering::SeqCst);
        let payload = JsonRpcRequest {
            id,
            jsonrpc: "2.0",
            request,
        };
        let text = self
            .client
            .post(self.url.as_ref())
            .json(&payload)
            .send()
            .await?
            .text()
            .await?;

        let response: JsonRpcResponse = serde_json::from_str(&text)?;

        Ok(response.result)
    }
}

impl FromStr for Http {
    type Err = url::ParseError;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(src)?;
        Ok(Http::new(url))
    }
}

impl Clone for Http {
    fn clone(&self) -> Self {
        Self {
            id: AtomicU64::new(0),
            client: self.client.clone(),
            url: self.url.clone(),
        }
    }
}
