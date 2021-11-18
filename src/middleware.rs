use std::fmt::Debug;

use async_trait::async_trait;
use ethers::prelude::{
    transaction::{eip2718::TypedTransaction, eip2930::AccessListWithGasUsed},
    *,
};
use serde_json::Value;

use crate::{
    error::RpcError,
    filter_watcher::{LogWatcher, NewBlockWatcher, PendingTransactionWatcher},
    pending_transaction::PendingTransaction,
    provider::RpcConnection,
};

/// Exposes RPC methods shared by all clients
#[async_trait]
pub trait Middleware: Debug + Send + Sync {
    #[doc(hidden)]
    fn inner(&self) -> &dyn Middleware;

    #[doc(hidden)]
    fn provider(&self) -> &dyn RpcConnection;

    /// Returns the current client version using the `web3_clientVersion` RPC.
    async fn client_version(&self) -> Result<String, RpcError> {
        self.inner().client_version().await
    }

    /// Gets the latest block number via the `eth_BlockNumber` API
    async fn get_block_number(&self) -> Result<U64, RpcError> {
        self.inner().get_block_number().await
    }

    /// Gets the block at `block_hash_or_number` (transaction hashes only)
    async fn get_block(
        &self,
        block_hash_or_number: BlockId,
    ) -> Result<Option<Block<TxHash>>, RpcError> {
        self.inner().get_block(block_hash_or_number).await
    }

    /// Gets the block at `block_hash_or_number` (full transactions included)
    async fn get_block_with_txs(
        &self,
        block_hash_or_number: BlockId,
    ) -> Result<Option<Block<Transaction>>, RpcError> {
        self.inner().get_block_with_txs(block_hash_or_number).await
    }

    /// Gets the block uncle count at `block_hash_or_number`
    async fn get_uncle_count(&self, block_hash_or_number: BlockId) -> Result<U256, RpcError> {
        self.inner().get_uncle_count(block_hash_or_number).await
    }

    /// Gets the block uncle at `block_hash_or_number` and `idx`
    async fn get_uncle(
        &self,
        block_hash_or_number: BlockId,
        idx: U64,
    ) -> Result<Option<Block<H256>>, RpcError> {
        self.inner().get_uncle(block_hash_or_number, idx).await
    }

    /// Gets the transaction with `transaction_hash`
    async fn get_transaction(
        &self,
        transaction_hash: TxHash,
    ) -> Result<Option<Transaction>, RpcError> {
        self.inner().get_transaction(transaction_hash).await
    }

    /// Gets the transaction receipt with `transaction_hash`
    async fn get_transaction_receipt(
        &self,
        transaction_hash: TxHash,
    ) -> Result<Option<TransactionReceipt>, RpcError> {
        self.inner().get_transaction_receipt(transaction_hash).await
    }

    /// Returns all receipts for a block.
    ///
    /// Note that this uses the `eth_getBlockReceipts` or `parity_getBlockReceipts` RPC, which is
    /// non-standard and currently supported by Erigon, OpenEthereum and Nethermind.
    async fn get_block_receipts(
        &self,
        block: BlockNumber,
    ) -> Result<Vec<TransactionReceipt>, RpcError> {
        self.inner().get_block_receipts(block).await
    }

    /// Gets the current gas price as estimated by the node
    async fn gas_price(&self) -> Result<U256, RpcError> {
        self.inner().gas_price().await
    }

    /// Gets the accounts on the node
    async fn accounts(&self) -> Result<Vec<Address>, RpcError> {
        self.inner().accounts().await
    }

    /// Returns the nonce of the address
    async fn get_transaction_count(
        &self,
        from: Address,
        block: Option<BlockNumber>,
    ) -> Result<U256, RpcError> {
        self.inner().get_transaction_count(from, block).await
    }

    /// Returns the account's balance
    async fn get_balance(
        &self,
        from: Address,
        block: Option<BlockNumber>,
    ) -> Result<U256, RpcError> {
        self.inner().get_balance(from, block).await
    }

    /// Returns the currently configured chain id, a value used in replay-protected
    /// transaction signing as introduced by EIP-155.
    async fn chain_id(&self) -> Result<U256, RpcError> {
        self.inner().chain_id().await
    }

    /// Sends the read-only (constant) transaction to a single Ethereum node and return the result
    /// (as bytes) of executing it. This is free, since it does not change any state on the
    /// blockchain.
    async fn call(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockNumber>,
    ) -> Result<Bytes, RpcError> {
        self.inner().call(tx, block).await
    }

    /// Sends a transaction to a single Ethereum node and return the estimated amount of gas
    /// required (as a U256) to send it This is free, but only an estimate. Providing too little
    /// gas will result in a transaction being rejected (while still consuming all provided
    /// gas).
    async fn estimate_gas(&self, tx: &TypedTransaction) -> Result<U256, RpcError> {
        self.inner().estimate_gas(tx).await
    }

    /// Create an EIP-2930 access list
    async fn create_access_list(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockNumber>,
    ) -> Result<AccessListWithGasUsed, RpcError> {
        self.inner().create_access_list(tx, block).await
    }

    /// Sends the transaction to the entire Ethereum network and returns the transaction's hash
    /// This will consume gas from the account that signed the transaction.
    async fn send_transaction(
        &self,
        tx: &TypedTransaction,
        block: Option<BlockNumber>,
    ) -> Result<PendingTransaction<'_>, RpcError> {
        self.inner().send_transaction(tx, block).await
    }

    /// Send the raw RLP encoded transaction to the entire Ethereum network and returns the
    /// transaction's hash This will consume gas from the account that signed the transaction.
    async fn send_raw_transaction(&self, tx: Bytes) -> Result<PendingTransaction<'_>, RpcError> {
        self.inner().send_raw_transaction(tx).await
    }

    /// Signs data using a specific account. This account needs to be unlocked.
    async fn sign(&self, from: Address, data: Bytes) -> Result<Signature, RpcError> {
        self.inner().sign(from, data).await
    }

    /// Returns an array (possibly empty) of logs that match the filter
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>, RpcError> {
        self.inner().get_logs(filter).await
    }

    /// Create a new block filter for later polling.
    async fn new_block_filter(&self) -> Result<U256, RpcError> {
        self.inner().new_block_filter().await
    }

    /// Create a stream that repeatedly polls a new block filter
    async fn watch_new_blocks(&self) -> Result<NewBlockWatcher, RpcError>
    where
        Self: Sized,
    {
        Ok(NewBlockWatcher::new(self.new_block_filter().await?, self))
    }

    /// Create a new pending transaction filter for later polling.
    async fn new_pending_transaction_filter(&self) -> Result<U256, RpcError> {
        self.inner().new_pending_transaction_filter().await
    }

    /// Create a stream that repeatedly polls a pending transaction filter
    async fn watch_new_pending_transactions(&self) -> Result<PendingTransactionWatcher, RpcError>
    where
        Self: Sized,
    {
        Ok(PendingTransactionWatcher::new(
            self.new_pending_transaction_filter().await?,
            self,
        ))
    }

    /// Create a new log filter for later polling.
    async fn new_log_filter(&self, filter: &Filter) -> Result<U256, RpcError> {
        self.inner().new_log_filter(filter).await
    }

    /// Create a stream that repeatedly polls a log filter
    async fn watch_new_logs(&self, filter: &Filter) -> Result<LogWatcher, RpcError>
    where
        Self: Sized,
    {
        Ok(LogWatcher::new(self.new_log_filter(filter).await?, self))
    }

    #[doc(hidden)]
    async fn get_filter_changes(&self, id: U256) -> Result<Vec<Value>, RpcError> {
        self.inner().get_filter_changes(id).await
    }

    /// Poll a pending transaction filter for any changes
    async fn poll_pending_transaction_filter(&self, id: U256) -> Result<Vec<TxHash>, RpcError> {
        self.get_filter_changes(id)
            .await?
            .into_iter()
            .map(|value| serde_json::from_value(value).map_err(Into::into))
            .collect()
    }

    /// Poll a new block filter for any changes
    async fn poll_new_block_filter(&self, id: U256) -> Result<Vec<H256>, RpcError> {
        self.get_filter_changes(id)
            .await?
            .into_iter()
            .map(|value| serde_json::from_value(value).map_err(Into::into))
            .collect()
    }

    /// Poll an event log filter for any changes
    async fn poll_log_filter(&self, id: U256) -> Result<Vec<Log>, RpcError> {
        self.get_filter_changes(id)
            .await?
            .into_iter()
            .map(|value| serde_json::from_value(value).map_err(Into::into))
            .collect()
    }

    // /// Uninstall a block, log, or pending transaction filter on the RPC host
    async fn uninstall_filter(&self, id: U256) -> Result<bool, RpcError> {
        self.inner().uninstall_filter(id).await
    }

    /// Get the storage of an address for a particular slot location
    async fn get_storage_at(
        &self,
        from: Address,
        location: H256,
        block: Option<BlockNumber>,
    ) -> Result<H256, RpcError> {
        self.inner().get_storage_at(from, location, block).await
    }

    /// Returns the deployed code at a given address
    async fn get_code(&self, at: Address, block: Option<BlockNumber>) -> Result<Bytes, RpcError> {
        self.inner().get_code(at, block).await
    }

    /// Returns the EIP-1186 proof response
    /// https://github.com/ethereum/EIPs/issues/1186
    async fn get_proof(
        &self,
        from: Address,
        locations: Vec<H256>,
        block: Option<BlockNumber>,
    ) -> Result<EIP1186ProofResponse, RpcError> {
        self.inner().get_proof(from, locations, block).await
    }

    /// Return the eip1559 RPC Fee History object
    async fn fee_history(
        &self,
        block_count: U256,
        last_block: BlockNumber,
        reward_percentiles: &[f64],
    ) -> Result<FeeHistory, RpcError> {
        self.inner()
            .fee_history(block_count, last_block, reward_percentiles)
            .await
    }
}

/// Exposes geth-specific RPC methods
#[async_trait]
pub trait GethMiddleware: Middleware + Send + Sync {
    /// Upcast the `GethMiddleware` to a generic `Middleware`
    fn as_middleware(&self) -> &dyn Middleware;

    #[doc(hidden)]
    fn inner_geth(&self) -> &dyn GethMiddleware;

    /// Returns the details of all transactions currently pending for inclusion in the next
    /// block(s), as well as the ones that are being scheduled for future execution only.
    /// Ref: [Here](https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_content)
    async fn txpool_content(&self) -> Result<TxpoolContent, RpcError> {
        self.inner_geth().txpool_content().await
    }

    /// Returns a summary of all the transactions currently pending for inclusion in the next
    /// block(s), as well as the ones that are being scheduled for future execution only.
    /// Ref: [Here](https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_inspect)
    async fn txpool_inspect(&self) -> Result<TxpoolInspect, RpcError> {
        self.inner_geth().txpool_inspect().await
    }

    /// Returns the number of transactions currently pending for inclusion in the next block(s), as
    /// well as the ones that are being scheduled for future execution only.
    /// Ref: [Here](https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_status)
    async fn txpool_status(&self) -> Result<TxpoolStatus, RpcError> {
        self.inner_geth().txpool_status().await
    }
}

/// Exposes parity (openethereum)-specific RPC methods
#[async_trait]
pub trait ParityMiddleware: Middleware + Send + Sync {
    /// Upcast the `ParityMiddleware` to a generic `Middleware`
    fn as_middleware(&self) -> &dyn Middleware;

    #[doc(hidden)]
    fn inner_parity(&self) -> &dyn ParityMiddleware;

    /// Executes the given call and returns a number of possible traces for it
    async fn trace_call(
        &self,
        req: TypedTransaction,
        trace_type: Vec<TraceType>,
        block: Option<BlockNumber>,
    ) -> Result<BlockTrace, RpcError> {
        self.inner_parity().trace_call(req, trace_type, block).await
    }

    /// Traces a call to `eth_sendRawTransaction` without making the call, returning the traces
    async fn trace_raw_transaction(
        &self,
        data: Bytes,
        trace_type: Vec<TraceType>,
    ) -> Result<BlockTrace, RpcError> {
        self.inner_parity()
            .trace_raw_transaction(data, trace_type)
            .await
    }

    /// Replays a transaction, returning the traces
    async fn trace_replay_transaction(
        &self,
        hash: H256,
        trace_type: Vec<TraceType>,
    ) -> Result<BlockTrace, RpcError> {
        self.inner_parity()
            .trace_replay_transaction(hash, trace_type)
            .await
    }

    /// Replays all transactions in a block returning the requested traces for each transaction
    async fn trace_replay_block_transactions(
        &self,
        block: BlockNumber,
        trace_type: Vec<TraceType>,
    ) -> Result<Vec<BlockTrace>, RpcError> {
        self.inner_parity()
            .trace_replay_block_transactions(block, trace_type)
            .await
    }

    /// Returns traces created at given block
    async fn trace_block(&self, block: BlockNumber) -> Result<Vec<Trace>, RpcError> {
        self.inner_parity().trace_block(block).await
    }

    /// Return traces matching the given filter
    async fn trace_filter(&self, filter: TraceFilter) -> Result<Vec<Trace>, RpcError> {
        self.inner_parity().trace_filter(filter).await
    }

    /// Returns trace at the given position
    async fn trace_get(&self, hash: H256, index: Vec<U64>) -> Result<Trace, RpcError> {
        self.inner_parity().trace_get(hash, index).await
    }

    /// Returns all traces of a given transaction
    async fn trace_transaction(&self, hash: H256) -> Result<Vec<Trace>, RpcError> {
        self.inner_parity().trace_transaction(hash).await
    }
}
