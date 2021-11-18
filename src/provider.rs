use std::fmt::Debug;

use async_trait::async_trait;
use ethers::prelude::U64;

use crate::{
    error::RpcError,
    middleware::Middleware,
    types::{RawRequest, RawResponse, RequestParams},
};

#[async_trait]
pub trait RpcConnection: Debug + Send + Sync {
    async fn _request(&self, request: RawRequest<'_>) -> Result<RawResponse, RpcError>;

    async fn request<T>(&self, params: T) -> Result<T::Response, RpcError>
    where
        Self: Sized,
        T: RequestParams,
    {
        let resp = self._request(params.to_raw_request()).await?;
        match resp {
            RawResponse::Success { result } => Ok(serde_json::from_value(result)?),
            RawResponse::Error { error } => Err(RpcError::ErrorResponse(error)),
        }
    }
}

#[async_trait]
impl<T> Middleware for T
where
    T: RpcConnection,
{
    fn inner(&self) -> &dyn Middleware {
        self
    }

    fn provider(&self) -> &dyn RpcConnection {
        self
    }

    async fn get_block_number(&self) -> Result<U64, RpcError> {
        crate::rpc::BlockNumberParams.send_via(self).await
    }
}

#[cfg(test)]
mod test {
    use core::panic;

    use super::*;
    use crate::transports::http::Http;

    #[tokio::test]
    async fn it_makes_a_req() {
        let provider: Http = "https://mainnet.infura.io/v3/5cfdec76313b457cb696ff1b89cee7ee"
            .parse()
            .unwrap();
        dbg!(provider.get_block_number().await.unwrap());
    }
}
