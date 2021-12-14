use crate::{error::RpcError, types::RawRequest, Network};

/// A middleware supporting development-specific JSON RPC methods
///
/// # Example
///
///```
/// use ethers_providers::{Provider, Http, Middleware, DevRpcMiddleware};
/// use ethers_core::types::TransactionRequest;
/// use ethers_core::utils::Ganache;
/// use std::convert::TryFrom;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let ganache = Ganache::new().spawn();
/// let provider : Http = ganache.endpoint().parse().unwrap();
/// let client = DevRpcMiddleware::new(provider);
///
/// // snapshot the initial state
/// let block0 = client.get_block_number().await.unwrap();
/// let snap_id = client.snapshot().await.unwrap();
///
/// // send a transaction
/// let accounts = client.get_accounts().await?;
/// let from = accounts[0];
/// let to = accounts[1];
/// let balance_before = client.get_balance(to, None).await?;
/// let tx = TransactionRequest::new().to(to).value(1000).from(from);
/// client.send_transaction(tx, None).await?.await?;
/// let balance_after = client.get_balance(to, None).await?;
/// assert_eq!(balance_after, balance_before + 1000);
///
/// // revert to snapshot
/// client.revert_to_snapshot(snap_id).await.unwrap();
/// let balance_after_revert = client.get_balance(to, None).await?;
/// assert_eq!(balance_after_revert, balance_before);
/// # Ok(())
/// # }
/// ```
use super::Middleware;
use ethers_core::types::U256;

use std::{fmt::Debug, ops::Deref};

#[derive(Debug)]
pub struct DevRpcMiddleware<N: Network>(Box<dyn Middleware<N>>);

impl<N: Network> Deref for DevRpcMiddleware<N> {
    type Target = Box<dyn Middleware<N>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<N: Network> DevRpcMiddleware<N> {
    pub fn new<M>(inner: M) -> Self
    where
        M: Middleware<N> + 'static,
    {
        Self(Box::new(inner))
    }

    pub async fn mine(&self, num_blocks: usize) -> Result<(), RpcError> {
        for _ in 0..num_blocks {
            self.0
                .provider()
                ._request(RawRequest::new("evm_mine", ())?)
                .await?;
        }
        Ok(())
    }

    // both ganache and hardhat increment snapshot id even if no state has changed
    pub async fn snapshot(&self) -> Result<U256, RpcError> {
        let request = RawRequest::new("evm_snapshot", ())?;

        Ok(self.0.provider()._request(request).await?.deserialize()?)
    }

    pub async fn revert_to_snapshot(&self, id: U256) -> Result<(), RpcError> {
        let request = RawRequest::new("evm_revert", &[id])?;

        // behavior of dropping all error info preserved from original
        let result = self.0.provider()._request(request).await?.into_result()?;

        match result {
            serde_json::Value::Bool(true) => Ok(()),
            serde_json::Value::Bool(false) => Err(RpcError::NoSnapshot),
            _ => panic!("evm_revert returned invalid value"),
        }
    }
}

#[cfg(test)]
// Celo blocks can not get parsed when used with Ganache
#[cfg(not(feature = "celo"))]
mod tests {
    use super::*;
    use crate::{Ethereum, Http};
    use ethers_core::utils::Ganache;

    #[tokio::test]
    async fn test_snapshot() {
        // launch ganache
        let ganache = Ganache::new().spawn();
        let provider: Http = ganache.endpoint().parse().unwrap();
        let client = DevRpcMiddleware::<Ethereum>::new(provider);

        // ensure the deref works
        client.get_block_number().await.unwrap();

        // snapshot initial state
        let block0 = client.get_block_number().await.unwrap();
        let time0 = client
            .get_block(block0.into())
            .await
            .unwrap()
            .unwrap()
            .timestamp;
        let snap_id0 = client.snapshot().await.unwrap();

        // mine a new block
        client.mine(1).await.unwrap();

        // snapshot state
        let block1 = client.get_block_number().await.unwrap();
        let time1 = client
            .get_block(block1.into())
            .await
            .unwrap()
            .unwrap()
            .timestamp;
        let snap_id1 = client.snapshot().await.unwrap();

        // mine some blocks
        client.mine(5).await.unwrap();

        // snapshot state
        let block2 = client.get_block_number().await.unwrap();
        let time2 = client
            .get_block(block2.into())
            .await
            .unwrap()
            .unwrap()
            .timestamp;
        let snap_id2 = client.snapshot().await.unwrap();

        // mine some blocks
        client.mine(5).await.unwrap();

        // revert_to_snapshot should reset state to snap id
        client.revert_to_snapshot(snap_id2).await.unwrap();
        let block = client.get_block_number().await.unwrap();
        let time = client
            .get_block(block.into())
            .await
            .unwrap()
            .unwrap()
            .timestamp;
        assert_eq!(block, block2);
        assert_eq!(time, time2);

        client.revert_to_snapshot(snap_id1).await.unwrap();
        let block = client.get_block_number().await.unwrap();
        let time = client
            .get_block(block.into())
            .await
            .unwrap()
            .unwrap()
            .timestamp;
        assert_eq!(block, block1);
        assert_eq!(time, time1);

        // revert_to_snapshot should throw given non-existent or
        // previously used snapshot
        let result = client.revert_to_snapshot(snap_id1).await;
        assert!(result.is_err());

        client.revert_to_snapshot(snap_id0).await.unwrap();
        let block = client.get_block_number().await.unwrap();
        let time = client
            .get_block(block.into())
            .await
            .unwrap()
            .unwrap()
            .timestamp;
        assert_eq!(block, block0);
        assert_eq!(time, time0);
    }
}
