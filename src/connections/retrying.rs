use std::{fmt::Debug, str::FromStr, time::Duration};

use async_trait::async_trait;
use tokio::time::sleep;
use tracing::{debug, instrument, warn};

use crate::{
    connections::{PubSubConnection, RpcConnection},
    error::RpcError,
    types::{RawRequest, RawResponse},
};

/// An HTTP Provider with a simple naive exponential backoff built-in
#[derive(Debug, Clone)]
pub struct RetryingProvider<P> {
    inner: P,
    max_requests: usize,
}

impl<P> RetryingProvider<P> {
    /// Instantiate a RetryingProvider
    pub fn new(inner: P, max_requests: usize) -> Self {
        Self {
            inner,
            max_requests,
        }
    }

    /// Set the max_requests (and by extension the total time a request can take)
    pub fn set_max_requests(&mut self, max_requests: usize) {
        self.max_requests = max_requests;
    }

    /// Get the max_requests
    pub fn get_max_requests(&self) -> usize {
        self.max_requests
    }
}

#[async_trait]
impl<P> RpcConnection for RetryingProvider<P>
where
    P: RpcConnection + 'static,
{
    #[instrument(
        level = "debug",
        err,
        skip(request),
        fields(request = %serde_json::to_string(&request).unwrap()))
    ]
    async fn _request(&self, request: RawRequest) -> Result<RawResponse, RpcError> {
        let mut errors = vec![];

        for i in 0..self.max_requests {
            let backoff_seconds = 2u64.pow(i as u32);
            {
                debug!(attempt = i, "Dispatching request");

                match self.inner._request(request.clone()).await {
                    Ok(res) => return Ok(res),
                    Err(e) => {
                        warn!(
                            backoff_seconds,
                            retries_remaining = self.max_requests - i - 1,
                            error = %e,
                            method = %request.method,
                            "Error in retrying provider",
                        );
                        errors.push(e);
                    }
                }
            }
            sleep(Duration::from_secs(backoff_seconds)).await;
        }

        return Err(RpcError::MaxRequests(Box::new(errors)));
    }
}

impl<P> FromStr for RetryingProvider<P>
where
    P: RpcConnection + FromStr,
{
    type Err = <P as FromStr>::Err;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(src.parse()?, 6))
    }
}

impl<P> PubSubConnection for RetryingProvider<P>
where
    P: PubSubConnection + 'static,
{
    fn uninstall_listener(&self, id: ethers::prelude::U256) -> Result<(), RpcError> {
        self.inner.uninstall_listener(id)
    }

    fn install_listener(
        &self,
        id: ethers::prelude::U256,
    ) -> Result<futures_channel::mpsc::UnboundedReceiver<crate::types::Notification>, RpcError>
    {
        self.inner.install_listener(id)
    }
}
