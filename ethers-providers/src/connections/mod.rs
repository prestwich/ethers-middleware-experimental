//! RPC Connections which can be used as the core of a middleware stack

mod http;
pub use http::Http;

mod ws;
pub use ws::Ws;

mod mock;
pub use mock::MockRpcConnection;

mod quorum;
pub use quorum::{Quorum, QuorumProvider, WeightedProvider};

mod retrying;
pub use retrying::RetryingProvider;

#[cfg(not(target_arch = "wasm32"))]
pub mod ipc;
#[cfg(not(target_arch = "wasm32"))]
pub use ipc::Ipc;

use ethers_core::{
    abi::ParamType,
    types::{
        transaction::{eip2718::TypedTransaction, eip2930::AccessListWithGasUsed},
        *,
    },
    utils::{
        eip1559_default_estimator, EIP1559_FEE_ESTIMATION_PAST_BLOCKS,
        EIP1559_FEE_ESTIMATION_REWARD_PERCENTILE,
    },
};
use futures_channel::mpsc::UnboundedReceiver;
use serde::Serialize;
use serde_json::Value;
use std::fmt::Debug;

use crate::{
    error::RpcError,
    middleware::{
        maybe, BaseMiddleware, GethMiddleware, Middleware, ParityMiddleware, PubSubMiddleware,
    },
    networks::{Network, TransactionRequest},
    rpc,
    subscriptions::{
        GenericLogStream, GenericNewBlockStream, GenericPendingTransactionStream,
        GenericSyncingStream,
    },
    types::{Eip1559Fees, NodeClient, Notification, RawRequest, RawResponse, RequestParams},
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

/// An `RpcConnection` represents the transport that dispatches requests to a
/// node and receives responses.
///
/// # Example
///
/// ```no_run
/// use ethers_providers::{RpcConnection, Http};
///
/// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// let provider: Http = "http://localhost:8545".parse()?;
/// let block_number = provider.call_method("eth_blockNumber", ())
///     .await?
///     .deserialize()?;
/// # Ok(())
/// # }
/// ```
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait RpcConnection: Debug + Send + Sync {
    /// Dispatch a request to the node
    async fn _request(&self, request: RawRequest) -> Result<RawResponse, RpcError>;

    /// Convenience function for dispatching simple RPC requests
    async fn call_method<S>(&self, method: &'static str, params: S) -> Result<RawResponse, RpcError>
    where
        S: Serialize + Send + Sync,
        Self: Sized,
    {
        let request = RawRequest::new(method, params)?;
        self._request(request).await
    }

    /// Convenience function for dispatching simple RPC requests
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

/// A Connection that supports real-time notifications, e.g. a WebSocket.
///
/// Users should never call functions on this trait directly, instead they
/// should be accessed via the types in the `subscriptions` module
pub trait PubSubConnection: RpcConnection + Send + Sync {
    #[doc(hidden)]
    fn uninstall_listener(&self, id: U256) -> Result<(), RpcError>;

    #[doc(hidden)]
    fn install_listener(&self, id: U256) -> Result<UnboundedReceiver<Notification>, RpcError>;
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl<T, N> BaseMiddleware<N> for T
where
    T: RpcConnection,
    N: Network,
{
    fn inner_base(&self) -> &dyn BaseMiddleware<N> {
        self
    }

    fn provider(&self) -> &dyn RpcConnection {
        self
    }

    async fn node_client(&self) -> Result<NodeClient, RpcError> {
        Ok(BaseMiddleware::<N>::client_version(self)
            .await?
            .parse()
            .expect("infallible parse"))
    }

    async fn client_version(&self) -> Result<String, RpcError> {
        rpc::dispatch_client_version(self).await
    }

    async fn net_version(&self) -> Result<U64, RpcError> {
        rpc::dispatch_net_version(self).await
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
        if BaseMiddleware::<N>::node_client(self)
            .await?
            .parity_block_receipts()
        {
            rpc::dispatch_parity_get_block_receipts(self, &block.into()).await
        } else {
            rpc::dispatch_get_block_receipts(self, &block.into()).await
        }
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
        tx: &N::TransactionRequest,
        block: Option<BlockNumber>,
    ) -> Result<Bytes, RpcError> {
        let block = block.unwrap_or(BlockNumber::Latest);
        rpc::dispatch_call(self, &rpc::CallParams(serde_json::to_value(tx)?, block)).await
    }

    /// Sends a transaction to a single Ethereum node and return the estimated amount of gas
    /// required (as a U256) to send it This is free, but only an estimate. Providing too little
    /// gas will result in a transaction being rejected (while still consuming all provided
    /// gas).
    async fn estimate_gas(&self, tx: &N::TransactionRequest) -> Result<U256, RpcError> {
        rpc::dispatch_estimate_gas(self, &serde_json::to_value(tx)?.into()).await
    }

    /// Create an EIP-2930 access list
    async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: Option<BlockNumber>,
    ) -> Result<AccessListWithGasUsed, RpcError> {
        let block = block.unwrap_or(BlockNumber::Latest);

        rpc::dispatch_create_access_list(
            self,
            &rpc::CreateAccessListParams(serde_json::to_value(tx)?, block),
        )
        .await
    }

    async fn send_transaction(
        &self,
        tx: &N::TransactionRequest,
        block: Option<BlockNumber>,
    ) -> Result<TxHash, RpcError> {
        let _block = block.unwrap_or(BlockNumber::Latest);

        rpc::dispatch_send_transaction(self, &serde_json::to_value(tx)?.into()).await
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
    /// <https://github.com/ethereum/EIPs/issues/1186>
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
        // try the proper way first
        let first = rpc::dispatch_fee_history(
            self,
            &rpc::FeeHistoryParams(block_count, last_block, reward_percentiles.to_vec()),
        )
        .await;

        // only try the legacy way if the good way doesn't work
        if first.is_err() {
            rpc::dispatch_legacy_fee_history(
                self,
                &rpc::LegacyFeeHistoryParams(
                    block_count.low_u64(),
                    last_block,
                    reward_percentiles.to_vec(),
                ),
            )
            .await
        } else {
            first
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl<T, N> ParityMiddleware<N> for T
where
    T: RpcConnection,
    N: Network,
{
    fn inner_parity(&self) -> &dyn ParityMiddleware<N> {
        self
    }

    fn as_base_middleware(&self) -> &dyn BaseMiddleware<N> {
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

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl<T, N> GethMiddleware<N> for T
where
    T: RpcConnection,
    N: Network,
{
    fn inner_geth(&self) -> &dyn GethMiddleware<N> {
        self
    }

    fn as_base_middleware(&self) -> &dyn BaseMiddleware<N> {
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

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl<T, N> Middleware<N> for T
where
    T: RpcConnection,
    N: Network,
{
    fn inner(&self) -> &dyn Middleware<N> {
        self
    }

    fn as_base_middleware(&self) -> &dyn BaseMiddleware<N> {
        self
    }

    fn as_geth_middleware(&self) -> &dyn GethMiddleware<N> {
        self
    }

    fn as_parity_middleware(&self) -> &dyn ParityMiddleware<N> {
        self
    }

    /// Helper for filling a transaction
    async fn fill_transaction(
        &self,
        tx: &mut N::TransactionRequest,
        block: Option<BlockNumber>,
    ) -> Result<(), RpcError> {
        if let Some(default_sender) = BaseMiddleware::<N>::default_sender(self) {
            if tx.from().is_none() {
                tx.set_from(default_sender);
            }
        }

        // TODO: Can we poll the futures below at the same time?
        // Access List + Name resolution and then Gas price + Gas

        // set the ENS name
        if let Some(NameOrAddress::Name(ref ens_name)) = tx.to() {
            let addr = Middleware::<N>::ens_resolve(self, None, ens_name).await?;
            tx.set_to(addr);
        }

        // estimate the gas without the access list
        let gas = maybe(
            tx.gas().cloned(),
            BaseMiddleware::<N>::estimate_gas(self, tx),
        )
        .await?;
        let mut al_used = false;

        // set the access lists
        if let Some(access_list) = tx.access_list() {
            if access_list.0.is_empty() {
                if let Ok(al_with_gas) =
                    BaseMiddleware::<N>::create_access_list(self, tx, block).await
                {
                    // only set the access list if the used gas is less than the
                    // normally estimated gas
                    if al_with_gas.gas_used < gas {
                        tx.set_access_list(al_with_gas.access_list);
                        tx.set_gas(al_with_gas.gas_used);
                        al_used = true;
                    }
                }
            }
        }

        if !al_used {
            tx.set_gas(gas);
        }

        if tx.recommend_1559() {
            let fees = tx.get_1559_fees();

            if fees.max_fee_per_gas.is_none() || fees.max_priority_fee_per_gas.is_none() {
                let (max_fee_per_gas, max_priority_fee_per_gas) =
                    Middleware::<N>::estimate_eip1559_fees(self, None).await?;

                let fees = Eip1559Fees {
                    max_fee_per_gas: Some(max_fee_per_gas),
                    max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
                };

                tx.set_1559_fees(&fees);
            };
        } else {
            let gas_price = maybe(tx.gas_price(), BaseMiddleware::<N>::gas_price(self)).await?;
            tx.set_gas_price(gas_price);
        }

        Ok(())
    }

    async fn ens_resolve(
        &self,
        registry: Option<Address>,
        ens_name: &str,
    ) -> Result<Address, RpcError> {
        Middleware::<N>::query_resolver(
            self,
            registry,
            ParamType::Address,
            ens_name,
            crate::ens::ADDR_SELECTOR,
        )
        .await
    }

    async fn ens_lookup(
        &self,
        registry: Option<Address>,
        address: Address,
    ) -> Result<String, RpcError> {
        let ens_name = crate::ens::reverse_address(address);

        Middleware::<N>::query_resolver(
            self,
            registry,
            ParamType::String,
            &ens_name,
            crate::ens::NAME_SELECTOR,
        )
        .await
    }

    async fn estimate_eip1559_fees(
        &self,
        estimator: Option<fn(U256, Vec<Vec<U256>>) -> (U256, U256)>,
    ) -> Result<(U256, U256), RpcError> {
        let this = Middleware::<N>::as_base_middleware(self);
        let base_fee_per_gas = this
            .get_block(BlockNumber::Latest.into())
            .await?
            .ok_or_else(|| RpcError::CustomError("Latest block not found".into()))?
            .base_fee_per_gas
            .ok_or_else(|| RpcError::CustomError("EIP-1559 not activated".into()))?;

        let fee_history = this
            .fee_history(
                EIP1559_FEE_ESTIMATION_PAST_BLOCKS.into(),
                BlockNumber::Latest,
                &[EIP1559_FEE_ESTIMATION_REWARD_PERCENTILE],
            )
            .await?;

        // use the provided fee estimator function, or fallback to the default implementation.
        let (max_fee_per_gas, max_priority_fee_per_gas) = if let Some(es) = estimator {
            es(base_fee_per_gas, fee_history.reward)
        } else {
            eip1559_default_estimator(base_fee_per_gas, fee_history.reward)
        };

        Ok((max_fee_per_gas, max_priority_fee_per_gas))
    }

    async fn sign_transaction(
        &self,
        _: &N::TransactionRequest,
        _: Address,
    ) -> Result<Signature, RpcError> {
        Err(RpcError::SignerUnavailable)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl<T, N> PubSubMiddleware<N> for T
where
    T: PubSubConnection,
    N: Network,
{
    #[doc(hidden)]
    fn pubsub_provider(&self) -> &dyn PubSubConnection {
        self
    }

    #[doc(hidden)]
    fn inner_pubsub(&self) -> &dyn PubSubMiddleware<N> {
        self
    }

    #[doc(hidden)]
    fn as_middleware(&self) -> &dyn Middleware<N> {
        self
    }

    async fn subscribe_new_heads(&self) -> Result<U256, RpcError> {
        rpc::dispatch_subscribe_heads(self, &"newHeads".to_owned().into()).await
    }

    async fn subscribe_logs(&self, filter: &Filter) -> Result<U256, RpcError> {
        rpc::dispatch_subscribe_logs(
            self,
            &rpc::SubscribeLogsParams("logs".to_owned(), filter.clone()),
        )
        .await
    }

    async fn subscribe_new_pending_transactions(&self) -> Result<U256, RpcError> {
        rpc::dispatch_subscribe_new_pending_transactions(
            self,
            &"newPendingTransactions".to_owned().into(),
        )
        .await
    }

    async fn subscribe_syncing(&self) -> Result<U256, RpcError> {
        rpc::dispatch_subscribe_syncing(self, &"syncing".to_owned().into()).await
    }

    async fn stream_new_heads(&self) -> Result<GenericNewBlockStream<N>, RpcError> {
        let id = PubSubMiddleware::<N>::subscribe_new_heads(self).await?;
        GenericNewBlockStream::new(id, self)
    }

    async fn stream_logs(&self, filter: &Filter) -> Result<GenericLogStream<N>, RpcError> {
        let id = PubSubMiddleware::<N>::subscribe_logs(self, filter).await?;
        GenericLogStream::new(id, self)
    }

    async fn stream_new_pending_transactions(
        &self,
    ) -> Result<GenericPendingTransactionStream<N>, RpcError> {
        let id = PubSubMiddleware::<N>::subscribe_new_pending_transactions(self).await?;
        GenericPendingTransactionStream::new(id, self)
    }

    async fn stream_syncing(&self) -> Result<GenericSyncingStream<N>, RpcError> {
        let id = PubSubMiddleware::<N>::subscribe_syncing(self).await?;
        GenericSyncingStream::new(id, self)
    }

    async fn unsubscribe(&self, subscription_id: U256) -> Result<bool, RpcError> {
        self.uninstall_listener(subscription_id)?;
        rpc::dispatch_unsubscribe(self, &subscription_id.into()).await
    }
}
