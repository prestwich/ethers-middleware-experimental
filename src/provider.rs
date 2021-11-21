use ethers::{
    abi::ParamType,
    prelude::{
        transaction::{eip2718::TypedTransaction, eip2930::AccessListWithGasUsed},
        *,
    },
};
use serde_json::Value;
use std::fmt::Debug;

use async_trait::async_trait;

use crate::{
    error::RpcError,
    middleware::{GethMiddleware, Middleware, MiddlewareExt, ParityMiddleware},
    rpc,
    types::{RawRequest, RawResponse, RequestParams},
};

async fn get_block_gen(
    provider: &dyn RpcConnection,
    block_hash_or_number: BlockId,
    hydrate_txns: bool,
) -> Result<Option<Value>, RpcError> {
    match block_hash_or_number {
        BlockId::Hash(hash) => {
            rpc::dispatch_get_block_by_hash(
                provider,
                &rpc::GetBlockByHashParams(hash, hydrate_txns),
            )
            .await
        }
        BlockId::Number(number) => {
            rpc::dispatch_get_block_by_number(
                provider,
                &rpc::GetBlockByNumberParams(number, hydrate_txns),
            )
            .await
        }
    }
}

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

    async fn client_version(&self) -> Result<String, RpcError> {
        rpc::dispatch_client_version(self).await
    }

    async fn get_block_number(&self) -> Result<U64, RpcError> {
        rpc::dispatch_block_number(self).await
    }

    async fn get_block(
        &self,
        block_hash_or_number: BlockId,
    ) -> Result<Option<Block<TxHash>>, RpcError> {
        let resp = get_block_gen(self, block_hash_or_number, false).await?;
        Ok(resp.map(serde_json::from_value).transpose()?)
    }

    /// Gets the block at `block_hash_or_number` (full transactions included)
    async fn get_block_with_txs(
        &self,
        block_hash_or_number: BlockId,
    ) -> Result<Option<Block<Transaction>>, RpcError> {
        let resp = get_block_gen(self, block_hash_or_number, true).await?;
        Ok(resp.map(serde_json::from_value).transpose()?)
    }

    /// Gets the block uncle count at `block_hash_or_number`
    async fn get_uncle_count(&self, block_hash_or_number: BlockId) -> Result<U256, RpcError> {
        match block_hash_or_number {
            BlockId::Hash(hash) => rpc::dispatch_get_uncle_count_by_hash(self, &hash.into()).await,
            BlockId::Number(number) => {
                rpc::dispatch_get_uncle_count_by_block_number(self, &number.into()).await
            }
        }
    }

    /// Gets the block uncle at `block_hash_or_number` and `idx`
    async fn get_uncle(
        &self,
        block_hash_or_number: BlockId,
        idx: U64,
    ) -> Result<Option<Block<H256>>, RpcError> {
        match block_hash_or_number {
            BlockId::Hash(hash) => {
                rpc::dispatch_get_uncle_by_hash_and_index(
                    self,
                    &rpc::GetUncleByHashAndIndexParams(hash, idx),
                )
                .await
            }
            BlockId::Number(number) => {
                rpc::dispatch_get_uncle_by_block_number_and_index(
                    self,
                    &rpc::GetUncleByBlockNumberAndIndexParams(number, idx),
                )
                .await
            }
        }
    }

    /// Gets the transaction with `transaction_hash`
    async fn get_transaction(
        &self,
        transaction_hash: TxHash,
    ) -> Result<Option<Transaction>, RpcError> {
        rpc::dispatch_get_transaction_by_hash(self, &transaction_hash.into()).await
    }

    /// Gets the transaction receipt with `transaction_hash`
    async fn get_transaction_receipt(
        &self,
        transaction_hash: TxHash,
    ) -> Result<Option<TransactionReceipt>, RpcError> {
        rpc::dispatch_get_transaction_receipt(self, &transaction_hash.into()).await
    }

    /// Returns all receipts for a block.
    ///
    /// Note that this uses the `eth_getBlockReceipts` or `parity_getBlockReceipts` RPC, which is
    /// non-standard and currently supported by Erigon, OpenEthereum and Nethermind.
    async fn get_block_receipts(
        &self,
        block: BlockNumber,
    ) -> Result<Vec<TransactionReceipt>, RpcError> {
        rpc::dispatch_get_block_receipts(self, &block.into()).await
    }

    /// Gets the current gas price as estimated by the node
    async fn gas_price(&self) -> Result<U256, RpcError> {
        rpc::dispatch_gas_price(self).await
    }

    /// Gets the accounts on the node
    async fn accounts(&self) -> Result<Vec<Address>, RpcError> {
        rpc::dispatch_accounts(self).await
    }

    /// Returns the nonce of the address
    async fn get_transaction_count(
        &self,
        from: Address,
        block: Option<BlockNumber>,
    ) -> Result<U256, RpcError> {
        let block = block.unwrap_or(BlockNumber::Latest);
        rpc::dispatch_get_transaction_count(self, &rpc::GetTransactionCountParams(from, block))
            .await
    }

    /// Returns the account's balance
    async fn get_balance(
        &self,
        from: Address,
        block: Option<BlockNumber>,
    ) -> Result<U256, RpcError> {
        let block = block.unwrap_or(BlockNumber::Latest);
        rpc::dispatch_get_balance(self, &rpc::GetBalanceParams(from, block)).await
    }

    /// Returns the currently configured chain id, a value used in replay-protected
    /// transaction signing as introduced by EIP-155.
    async fn chain_id(&self) -> Result<U256, RpcError> {
        rpc::dispatch_chain_id(self).await
    }

    /// Sends the read-only (constant) transaction to a single Ethereum node and return the result
    /// (as bytes) of executing it. This is free, since it does not change any state on the
    /// blockchain.
    async fn call(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockNumber>,
    ) -> Result<Bytes, RpcError> {
        let block = block.unwrap_or(BlockNumber::Latest);
        rpc::dispatch_call(self, &rpc::CallParams(tx.clone(), block)).await
    }

    /// Sends a transaction to a single Ethereum node and return the estimated amount of gas
    /// required (as a U256) to send it This is free, but only an estimate. Providing too little
    /// gas will result in a transaction being rejected (while still consuming all provided
    /// gas).
    async fn estimate_gas(&self, tx: &TypedTransaction) -> Result<U256, RpcError> {
        rpc::dispatch_estimate_gas(self, &tx.clone().into()).await
    }

    /// Create an EIP-2930 access list
    async fn create_access_list(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockNumber>,
    ) -> Result<AccessListWithGasUsed, RpcError> {
        let block = block.unwrap_or(BlockNumber::Latest);

        rpc::dispatch_create_access_list(self, &rpc::CreateAccessListParams(tx.clone(), block))
            .await
    }

    async fn send_transaction(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockNumber>,
    ) -> Result<TxHash, RpcError> {
        let _block = block.unwrap_or(BlockNumber::Latest);

        // TODO: fill_transaction

        rpc::dispatch_send_transaction(self, &tx.clone().into()).await
    }

    async fn send_raw_transaction(&self, tx: Bytes) -> Result<TxHash, RpcError> {
        rpc::dispatch_send_raw_transaction(self, &tx.into()).await
    }

    /// Signs data using a specific account. This account needs to be unlocked.
    async fn sign(&self, from: Address, data: Bytes) -> Result<Signature, RpcError> {
        rpc::dispatch_sign(self, &rpc::SignParams(from, data)).await
    }

    /// Returns an array (possibly empty) of logs that match the filter
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>, RpcError> {
        rpc::dispatch_get_logs(self, &filter.clone().into()).await
    }

    /// Create a new block filter for later polling.
    async fn new_block_filter(&self) -> Result<U256, RpcError> {
        rpc::dispatch_new_block_filter(self).await
    }

    /// Create a new pending transaction filter for later polling.
    async fn new_pending_transaction_filter(&self) -> Result<U256, RpcError> {
        rpc::dispatch_new_pending_transaction_filter(self).await
    }

    /// Create a new log filter for later polling.
    async fn new_log_filter(&self, filter: &Filter) -> Result<U256, RpcError> {
        rpc::dispatch_new_filter(self, &filter.clone().into()).await
    }

    async fn get_filter_changes(&self, id: U256) -> Result<Vec<Value>, RpcError> {
        rpc::dispatch_get_filter_changes(self, &id.into()).await
    }

    // /// Uninstall a block, log, or pending transaction filter on the RPC host
    async fn uninstall_filter(&self, id: U256) -> Result<bool, RpcError> {
        rpc::dispatch_uninstall_filter(self, &id.into()).await
    }

    /// Get the storage of an address for a particular slot location
    async fn get_storage_at(
        &self,
        from: Address,
        location: H256,
        block: Option<BlockNumber>,
    ) -> Result<H256, RpcError> {
        let block = block.unwrap_or(BlockNumber::Latest);

        rpc::dispatch_get_storage_at(self, &rpc::GetStorageAtParams(from, location, block)).await
    }

    /// Returns the deployed code at a given address
    async fn get_code(&self, at: Address, block: Option<BlockNumber>) -> Result<Bytes, RpcError> {
        let block = block.unwrap_or(BlockNumber::Latest);
        rpc::dispatch_get_code(self, &rpc::GetCodeParams(at, block)).await
    }

    /// Returns the EIP-1186 proof response
    /// https://github.com/ethereum/EIPs/issues/1186
    async fn get_proof(
        &self,
        from: Address,
        locations: Vec<H256>,
        block: Option<BlockNumber>,
    ) -> Result<EIP1186ProofResponse, RpcError> {
        let block = block.unwrap_or(BlockNumber::Latest);
        rpc::dispatch_get_proof(self, &rpc::GetProofParams(from, locations, block)).await
    }

    /// Return the eip1559 RPC Fee History object
    async fn fee_history(
        &self,
        block_count: U256,
        last_block: BlockNumber,
        reward_percentiles: &[f64],
    ) -> Result<FeeHistory, RpcError> {
        rpc::dispatch_fee_history(
            self,
            &rpc::FeeHistoryParams(block_count, last_block, reward_percentiles.to_vec()),
        )
        .await
    }
}

#[async_trait]
impl<T> ParityMiddleware for T
where
    T: RpcConnection,
{
    fn inner_parity(&self) -> &dyn ParityMiddleware {
        self
    }

    fn as_middleware(&self) -> &dyn Middleware {
        self
    }

    /// Executes the given call and returns a number of possible traces for it
    async fn trace_call(
        &self,
        req: TypedTransaction,
        trace_type: Vec<TraceType>,
        block: Option<BlockNumber>,
    ) -> Result<BlockTrace, RpcError> {
        let block = block.unwrap_or(BlockNumber::Latest);
        rpc::dispatch_trace_call(self, &rpc::TraceCallParams(req, trace_type, block)).await
    }

    /// Traces a call to `eth_sendRawTransaction` without making the call, returning the traces
    async fn trace_raw_transaction(
        &self,
        data: Bytes,
        trace_type: Vec<TraceType>,
    ) -> Result<BlockTrace, RpcError> {
        rpc::dispatch_trace_raw_transaction(self, &rpc::TraceRawTransactionParams(data, trace_type))
            .await
    }

    /// Replays a transaction, returning the traces
    async fn trace_replay_transaction(
        &self,
        hash: H256,
        trace_type: Vec<TraceType>,
    ) -> Result<BlockTrace, RpcError> {
        rpc::dispatch_trace_replay_transaction(
            self,
            &rpc::TraceReplayTransactionParams(hash, trace_type),
        )
        .await
    }

    /// Replays all transactions in a block returning the requested traces for each transaction
    async fn trace_replay_block_transactions(
        &self,
        block: BlockNumber,
        trace_type: Vec<TraceType>,
    ) -> Result<Vec<BlockTrace>, RpcError> {
        rpc::dispatch_trace_replay_block_transactions(
            self,
            &rpc::TraceReplayBlockTransactionsParams(block, trace_type),
        )
        .await
    }

    /// Returns traces created at given block
    async fn trace_block(&self, block: BlockNumber) -> Result<Vec<Trace>, RpcError> {
        rpc::dispatch_trace_block(self, &block.into()).await
    }

    /// Return traces matching the given filter
    async fn trace_filter(&self, filter: TraceFilter) -> Result<Vec<Trace>, RpcError> {
        rpc::dispatch_trace_filter(self, &filter.into()).await
    }

    /// Returns trace at the given position
    async fn trace_get(&self, hash: H256, index: Vec<U64>) -> Result<Trace, RpcError> {
        rpc::dispatch_trace_get(self, &rpc::TraceGetParams(hash, index)).await
    }

    /// Returns all traces of a given transaction
    async fn trace_transaction(&self, hash: H256) -> Result<Vec<Trace>, RpcError> {
        rpc::dispatch_trace_transaction(self, &hash.into()).await
    }
}

#[async_trait]
impl<T> GethMiddleware for T
where
    T: RpcConnection,
{
    fn inner_geth(&self) -> &dyn GethMiddleware {
        self
    }

    fn as_middleware(&self) -> &dyn Middleware {
        self
    }

    async fn txpool_content(&self) -> Result<TxpoolContent, RpcError> {
        rpc::dispatch_txpool_content(self).await
    }

    async fn txpool_inspect(&self) -> Result<TxpoolInspect, RpcError> {
        rpc::dispatch_txpool_inspect(self).await
    }

    async fn txpool_status(&self) -> Result<TxpoolStatus, RpcError> {
        rpc::dispatch_txpool_status(self).await
    }
}

#[async_trait]
impl<T> MiddlewareExt for T
where
    T: RpcConnection,
{
    fn inner_ext(&self) -> &dyn MiddlewareExt {
        self
    }

    fn as_middleware(&self) -> &dyn Middleware {
        self
    }

    async fn ens_resolve(
        &self,
        registry: Option<Address>,
        ens_name: &str,
    ) -> Result<Address, RpcError> {
        self.query_resolver(registry, ParamType::Address, ens_name, ens::ADDR_SELECTOR)
            .await
    }

    async fn ens_lookup(
        &self,
        registry: Option<Address>,
        address: Address,
    ) -> Result<String, RpcError> {
        let ens_name = ens::reverse_address(address);

        self.query_resolver(registry, ParamType::String, &ens_name, ens::NAME_SELECTOR)
            .await
    }

    async fn sign_transaction(
        &self,
        _: &TypedTransaction,
        _: Address,
    ) -> Result<Signature, RpcError> {
        Err(RpcError::SignerUnavailable)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::transports::http::Http;

    #[derive(Debug)]
    struct DummyMiddleware;

    #[async_trait]
    impl Middleware for DummyMiddleware {
        fn inner(&self) -> &dyn Middleware {
            todo!()
        }

        fn provider(&self) -> &dyn RpcConnection {
            todo!()
        }

        async fn get_block_number(&self) -> Result<U64, RpcError> {
            Ok(0.into())
        }
    }

    #[tokio::test]
    async fn it_makes_a_req() {
        let provider: Http = "https://mainnet.infura.io/v3/5cfdec76313b457cb696ff1b89cee7ee"
            .parse()
            .unwrap();
        dbg!(provider.get_block_number().await.unwrap());
        let provider = Box::new(provider) as Box<dyn Middleware>;
        dbg!(provider.get_block_number().await.unwrap());
        let providers = vec![provider, Box::new(DummyMiddleware)];

        assert_eq!(providers[1].get_block_number().await.unwrap(), 0.into());
    }
}
