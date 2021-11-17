use std::fmt::Debug;

use async_trait::async_trait;
use ethers::prelude::U64;

use crate::{error::RpcError, provider::RpcConnection};

#[async_trait]
pub trait Middleware: Debug + Send + Sync {
    #[doc(hidden)]
    fn inner(&self) -> &dyn Middleware;

    #[doc(hidden)]
    fn provider(&self) -> &dyn RpcConnection;

    /// Fetch the current height of the chain
    async fn get_block_number(&self) -> Result<U64, RpcError> {
        self.inner().get_block_number().await
    }
}
