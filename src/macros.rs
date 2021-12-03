// used with WS and Quorum
macro_rules! if_wasm {
    ($($item:item)*) => {$(
        #[cfg(target_arch = "wasm32")]
        $item
    )*}
}

macro_rules! if_not_wasm {
    ($($item:item)*) => {$(
        #[cfg(not(target_arch = "wasm32"))]
        $item
    )*}
}

macro_rules! impl_rpc_params {
    ($method:literal, $params:ty, $res:ty) => {
        impl crate::types::RequestParams for $params {
            const METHOD: &'static str = $method;
            type Response = $res;
        }
    };
}

macro_rules! decl_rpc_param_type {
    ($method:literal, $name:ident) => {
        paste::paste! {
            #[doc = "RPC Params for `" $method "`"]
            #[derive(Debug, Copy, Clone, serde::Serialize)]
            pub struct [<$name Params>];
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ] ) => {
        paste::paste! {
            #[doc = "RPC Params for `" $method "`"]
            #[derive(Debug, serde::Serialize)]

            pub struct [<$name Params>]  (
                $(pub(crate) $param),*
            );
        }
    };

    ($method:literal, $name:ident, param: $param:ty) => {
        paste::paste! {
            #[doc = "RPC Params for `" $method "`"]
            #[derive(Debug, Clone)]
            pub struct [<$name Params>] ( pub(crate) $param );

            impl From<$param> for [<$name Params>] {
                fn from(p: $param) -> Self {
                    Self(p)
                }
            }

            impl serde::Serialize for [<$name Params>] {
                fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                    [&self.0].serialize(serializer)
                }
            }
        }
    };
}

macro_rules! impl_dispatch_method {
    ($name:ident, $resp:ty) => {
        paste::paste!{
            pub(crate) async fn [<dispatch_ $name:snake>](provider: &dyn crate::connections::RpcConnection) -> Result<Response, crate::error::RpcError> {
                use crate::types::RequestParams;
                [<$name Params>].send_via(provider).await
            }
        }
    };
    ($name:ident, $params:ty, $resp:ty) => {
        paste::paste!{
            pub(crate) async fn [<dispatch_ $name:snake>](provider: &dyn crate::connections::RpcConnection, params: &Params) -> Result<Response,crate::error::RpcError> {
                use crate::types::RequestParams;
                params.send_via(provider).await
            }
        }
    };
}

// // Currently unused, as we never need to in-line declare a response type.
// // All responses have existing types in ethers as far as I can tell
// macro_rules! decl_rpc_response_type {
//     ($method:literal, $name:ident, { $( $resp:ident: $resp_ty:ty, )* }) => {
//         paste::paste! {
//             #[doc = "RPC Response for `" $method "`"]
//             #[derive(Debug, serde::Deserialize)]
//             pub struct [<$name Response>]  {
//                 $($resp: $resp_ty,)*
//             }
//         }
//     };
// }

macro_rules! impl_rpc {
    ($method:literal, $name:ident, response: $resp:ty $(,)?) => {
        paste::paste! {
            #[allow(unused_imports)]
            mod [<inner_ $name:snake>] {
                use super::*;
                decl_rpc_param_type!($method, $name);

                type Params = [<$name Params>];
                type Response = $resp;

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Response);
            }
            pub use [<inner_ $name:snake>]::*;
        }
    };

    ($method:literal, $name:ident, response: { $( $resp:ident: $resp_ty:ty, )* $(,)?}) => {
        paste::paste! {
            mod [<inner_ $name:snake>] {
                use super::*;

                decl_rpc_param_type!($method, $name);
                decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* });

                type Params = [<$name Params>];
                type Response = [<$name Response>];

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Response);
            }
            pub use [<inner_ $name:snake>]::*;
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ], response: $resp:ty $(,)?) => {
        paste::paste! {
            mod [<inner_ $name:snake>] {
                use super::*;
                decl_rpc_param_type!($method, $name, params: [ $($param),* ]);

                type Params = [<$name Params>];
                type Response = $resp;

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Params, Response);
            }
            pub use [<inner_ $name:snake>]::*;
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ], response: { $( $resp:ident: $resp_ty:ty, )* } $(,)?) => {
        paste::paste! {
            mod [<inner_ $name:snake>] {
                use super::*;
                decl_rpc_param_type!($method, $name, params: [ $($param),* ]);
                decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* } );

                type Params = [<$name Params>];
                type Response = [<$name Response>];

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Params, Response);
            }
            pub use [<inner_ $name:snake>]::*;
        }
    };

    ($method:literal, $name:ident, param: $param:ty, response: { $( $resp:ident: $resp_ty:ty, )* } $(,)?) => {
        paste::paste! {
            mod [<inner_ $name:snake>] {
                use super::*;

                decl_rpc_param_type!($method, $name, param: $param);
                decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* });

                type Params = [<$name Params>];
                type Response = [<$name Response>];

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Params, Response);
            }
            pub use [<inner_ $name:snake>]::*;
        }
    };

    ($method:literal, $name:ident, param: $param:ty, response: $resp:ty $(,)?) => {
        paste::paste! {
            mod [<inner_ $name:snake>] {
                use super::*;
                decl_rpc_param_type!($method, $name, param: $param);

                type Params = [<$name Params>];
                type Response = $resp;

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Params, Response);
            }
            pub use [<inner_ $name:snake>]::*;
        }
    };
}

macro_rules! impl_network_middleware {
    ($network:ty) => {
        paste::paste! {
            #[async_trait::async_trait]
            #[doc = "Middleware for the `" $network  "` network"]
            pub trait [<$network Middleware>]:
                crate::middleware::BaseMiddleware<$network>
                + crate::middleware::GethMiddleware<$network>
                + crate::middleware::ParityMiddleware<$network>
                + crate::middleware::Middleware<$network>
                + std::fmt::Debug
                + Send
                + Sync
            {
                #[doc(hidden)]
                fn as_base_middleware(&self) -> &dyn BaseMiddleware<$network>;
                #[doc(hidden)]
                fn as_geth_middleware(&self) -> &dyn GethMiddleware<$network>;
                #[doc(hidden)]
                fn as_parity_middleware(&self) -> &dyn ParityMiddleware<$network>;
                #[doc(hidden)]
                fn as_middleware(&self) -> &dyn Middleware<$network>;


                /// Return a default tx sender address for this provider
                fn default_sender(&self) -> Option<ethers::prelude::Address> {
                    [<$network Middleware>]::as_base_middleware(self).default_sender()
                }

                /// Returns the current client version using the `web3_clientVersion` RPC.
                async fn client_version(&self) -> Result<String, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).client_version().await
                }

                /// Gets the latest block number via the `eth_BlockNumber` API
                async fn get_block_number(&self) -> Result<ethers::prelude::U64, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).get_block_number().await
                }

                /// Gets the block at `block_hash_or_number` (transaction hashes only)
                async fn get_block(
                    &self,
                    block_hash_or_number: ethers::prelude::BlockId,
                ) -> Result<Option<ethers::prelude::Block<ethers::prelude::TxHash>>, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).get_block(block_hash_or_number).await
                }

                /// Gets the block at `block_hash_or_number` (full transactions included)
                async fn get_block_with_txs(
                    &self,
                    block_hash_or_number: ethers::prelude::BlockId,
                ) -> Result<Option<ethers::prelude::Block<ethers::prelude::Transaction>>, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self)
                        .get_block_with_txs(block_hash_or_number)
                        .await
                }

                /// Gets the block uncle count at `block_hash_or_number`
                async fn get_uncle_count(&self, block_hash_or_number: ethers::prelude::BlockId) -> Result<U256, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self)
                        .get_uncle_count(block_hash_or_number)
                        .await
                }

                /// Gets the block uncle at `block_hash_or_number` and `idx`
                async fn get_uncle(
                    &self,
                    block_hash_or_number: ethers::prelude::BlockId,
                    idx: ethers::prelude::U64,
                ) -> Result<Option<ethers::prelude::Block<ethers::prelude::H256>>, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).get_uncle(block_hash_or_number, idx).await
                }

                /// Gets the transaction with `transaction_hash`
                async fn get_transaction(
                    &self,
                    transaction_hash: TxHash,
                ) -> Result<Option<ethers::prelude::Transaction>, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).get_transaction(transaction_hash).await
                }

                /// Gets the transaction receipt with `transaction_hash`
                async fn get_transaction_receipt(
                    &self,
                    transaction_hash: TxHash,
                ) -> Result<Option<ethers::prelude::TransactionReceipt>, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self)
                        .get_transaction_receipt(transaction_hash)
                        .await
                }

                /// Returns all receipts for a block.
                ///
                /// Note that this uses the `eth_getBlockReceipts` or `parity_getBlockReceipts` RPC, which is
                /// non-standard and currently supported by Erigon, OpenEthereum and Nethermind.
                async fn get_block_receipts(
                    &self,
                    block: ethers::prelude::BlockNumber,
                ) -> Result<Vec<ethers::prelude::TransactionReceipt>, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).get_block_receipts(block).await
                }

                /// Gets the current gas price as estimated by the node
                async fn gas_price(&self) -> Result<ethers::prelude::U256, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).gas_price().await
                }

                /// Gets the accounts on the node
                async fn accounts(&self) -> Result<Vec<ethers::prelude::Address>, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).accounts().await
                }

                /// Returns the nonce of the address
                async fn get_transaction_count(
                    &self,
                    from: Address,
                    block: Option<ethers::prelude::BlockNumber>,
                ) -> Result<ethers::prelude::U256, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).get_transaction_count(from, block).await
                }

                /// Returns the account's balance
                async fn get_balance(
                    &self,
                    from: Address,
                    block: Option<ethers::prelude::BlockNumber>,
                ) -> Result<ethers::prelude::U256, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).get_balance(from, block).await
                }

                /// Returns the currently configured chain id, a value used in replay-protected
                /// transaction signing as introduced by EIP-155.
                async fn chain_id(&self) -> Result<ethers::prelude::U256, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).chain_id().await
                }

                /// Sends the read-only (constant) transaction to a single Ethereum node and return the result
                /// (as bytes) of executing it. This is free, since it does not change any state on the
                /// blockchain.
                async fn call(
                    &self,
                    tx: &<$network as crate::networks::Network>::TransactionRequest,
                    block: Option<ethers::prelude::BlockNumber>,
                ) -> Result<Bytes, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).call(tx, block).await
                }

                /// Sends a transaction to a single Ethereum node and return the estimated amount of gas
                /// required (as a U256) to send it This is free, but only an estimate. Providing too little
                /// gas will result in a transaction being rejected (while still consuming all provided
                /// gas).
                async fn estimate_gas(&self, tx: &<$network as crate::networks::Network>::TransactionRequest) -> Result<ethers::prelude::U256, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).estimate_gas(tx).await
                }

                /// Create an EIP-2930 access list
                async fn create_access_list(
                    &self,
                    tx: &<$network as crate::networks::Network>::TransactionRequest,
                    block: Option<ethers::prelude::BlockNumber>,
                ) -> Result<ethers::core::types::transaction::eip2930::AccessListWithGasUsed, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).create_access_list(tx, block).await
                }

                /// Signs data using a specific account. This account needs to be unlocked.
                async fn sign(&self, from: Address, data: Bytes) -> Result<Signature, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).sign(from, data).await
                }

                /// Returns an array (possibly empty) of logs that match the filter
                async fn get_logs(&self, filter: &ethers::prelude::Filter) -> Result<Vec<ethers::prelude::Log>, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).get_logs(filter).await
                }

                /// Create a new block filter for later polling.
                async fn new_block_filter(&self) -> Result<ethers::prelude::U256, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).new_block_filter().await
                }

                /// Create a new pending transaction filter for later polling.
                async fn new_pending_transaction_filter(&self) -> Result<ethers::prelude::U256, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).new_pending_transaction_filter().await
                }

                /// Create a new log filter for later polling.
                async fn new_log_filter(&self, filter: &ethers::prelude::Filter) -> Result<ethers::prelude::U256, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).new_log_filter(filter).await
                }

                /// Poll a pending transaction filter for any changes
                async fn poll_pending_transaction_filter(&self, id: ethers::prelude::U256) -> Result<Vec<ethers::prelude::TxHash>, crate::error::RpcError> {
                    self.get_filter_changes(id)
                        .await?
                        .into_iter()
                        .map(|value| serde_json::from_value(value).map_err(Into::into))
                        .collect()
                }

                /// Poll a new block filter for any changes
                async fn poll_new_block_filter(&self, id: ethers::prelude::U256) -> Result<Vec<ethers::prelude::H256>, crate::error::RpcError> {
                    self.get_filter_changes(id)
                        .await?
                        .into_iter()
                        .map(|value| serde_json::from_value(value).map_err(Into::into))
                        .collect()
                }

                /// Poll an event log filter for any changes
                async fn poll_log_filter(&self, id: ethers::prelude::U256) -> Result<Vec<ethers::prelude::Log>, crate::error::RpcError> {
                    self.get_filter_changes(id)
                        .await?
                        .into_iter()
                        .map(|value| serde_json::from_value(value).map_err(Into::into))
                        .collect()
                }

                // /// Uninstall a block, log, or pending transaction filter on the RPC host
                async fn uninstall_filter(&self, id: ethers::prelude::U256) -> Result<bool, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).uninstall_filter(id).await
                }

                /// Get the storage of an address for a particular slot location
                async fn get_storage_at(
                    &self,
                    from: ethers::prelude::Address,
                    location: ethers::prelude::H256,
                    block: Option<ethers::prelude::BlockNumber>,
                ) -> Result<ethers::prelude::H256, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self)
                        .get_storage_at(from, location, block)
                        .await
                }

                /// Returns the deployed code at a given address
                async fn get_code(&self, at: ethers::prelude::Address, block: Option<ethers::prelude::BlockNumber>) -> Result<ethers::prelude::Bytes, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).get_code(at, block).await
                }

                /// Returns the EIP-1186 proof response
                /// https://github.com/ethereum/EIPs/issues/1186
                async fn get_proof(
                    &self,
                    from: ethers::prelude::Address,
                    locations: Vec<ethers::prelude::H256>,
                    block: Option<ethers::prelude::BlockNumber>,
                ) -> Result<ethers::prelude::EIP1186ProofResponse, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self).get_proof(from, locations, block).await
                }

                /// Return the eip1559 RPC Fee History object
                async fn fee_history(
                    &self,
                    block_count: ethers::prelude::U256,
                    last_block: ethers::prelude::BlockNumber,
                    reward_percentiles: &[f64],
                ) -> Result<ethers::prelude::FeeHistory, crate::error::RpcError> {
                    [<$network Middleware>]::as_base_middleware(self)
                        .fee_history(block_count, last_block, reward_percentiles)
                        .await
                }

                /// Returns the details of all transactions currently pending for inclusion in the next
                /// block(s), as well as the ones that are being scheduled for future execution only.
                /// Ref: [Here](https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_content)
                async fn txpool_content(&self) -> Result<ethers::prelude::TxpoolContent, crate::error::RpcError> {
                    [<$network Middleware>]::as_geth_middleware(self).txpool_content().await
                }

                /// Returns a summary of all the transactions currently pending for inclusion in the next
                /// block(s), as well as the ones that are being scheduled for future execution only.
                /// Ref: [Here](https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_inspect)
                async fn txpool_inspect(&self) -> Result<ethers::prelude::TxpoolInspect, crate::error::RpcError> {
                    [<$network Middleware>]::as_geth_middleware(self).txpool_inspect().await
                }

                /// Returns the number of transactions currently pending for inclusion in the next block(s), as
                /// well as the ones that are being scheduled for future execution only.
                /// Ref: [Here](https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_status)
                async fn txpool_status(&self) -> Result<ethers::prelude::TxpoolStatus, crate::error::RpcError> {
                    [<$network Middleware>]::as_geth_middleware(self).txpool_status().await
                }

                /// Executes the given call and returns a number of possible traces for it
                async fn trace_call(
                    &self,
                    req: <$network as crate::networks::Network >::TransactionRequest,
                    trace_type: Vec<ethers::prelude::TraceType>,
                    block: Option<ethers::prelude::BlockNumber>,
                ) -> Result<ethers::prelude::BlockTrace, crate::error::RpcError> {
                    [<$network Middleware>]::as_parity_middleware(self).trace_call(req, trace_type, block).await
                }

                /// Traces a call to `eth_sendRawTransaction` without making the call, returning the traces
                async fn trace_raw_transaction(
                    &self,
                    data: ethers::prelude::Bytes,
                    trace_type: Vec<ethers::prelude::TraceType>,
                ) -> Result<ethers::prelude::BlockTrace, crate::error::RpcError> {
                    [<$network Middleware>]::as_parity_middleware(self)
                        .trace_raw_transaction(data, trace_type)
                        .await
                }

                /// Replays a transaction, returning the traces
                async fn trace_replay_transaction(
                    &self,
                    hash: ethers::prelude::H256,
                    trace_type: Vec<ethers::prelude::TraceType>,
                ) -> Result<ethers::prelude::BlockTrace, crate::error::RpcError> {
                    [<$network Middleware>]::as_parity_middleware(self)
                        .trace_replay_transaction(hash, trace_type)
                        .await
                }

                /// Replays all transactions in a block returning the requested traces for each transaction
                async fn trace_replay_block_transactions(
                    &self,
                    block: ethers::prelude::BlockNumber,
                    trace_type: Vec<ethers::prelude::TraceType>,
                ) -> Result<Vec<ethers::prelude::BlockTrace>, crate::error::RpcError> {
                    [<$network Middleware>]::as_parity_middleware(self)
                        .trace_replay_block_transactions(block, trace_type)
                        .await
                }

                /// Returns traces created at given block
                async fn trace_block(&self, block: ethers::prelude::BlockNumber) -> Result<Vec<ethers::prelude::Trace>, crate::error::RpcError> {
                    [<$network Middleware>]::as_parity_middleware(self).trace_block(block).await
                }

                /// Return traces matching the given filter
                async fn trace_filter(&self, filter: ethers::prelude::TraceFilter) -> Result<Vec<ethers::prelude::Trace>, crate::error::RpcError> {
                    [<$network Middleware>]::as_parity_middleware(self).trace_filter(filter).await
                }

                /// Returns trace at the given position
                async fn trace_get(&self, hash: ethers::prelude::H256, index: Vec<ethers::prelude::U64>) -> Result<ethers::prelude::Trace, crate::error::RpcError> {
                    [<$network Middleware>]::as_parity_middleware(self).trace_get(hash, index).await
                }

                /// Returns all traces of a given transaction
                async fn trace_transaction(&self, hash: ethers::prelude::H256) -> Result<Vec<ethers::prelude::Trace>, crate::error::RpcError> {
                    [<$network Middleware>]::as_parity_middleware(self).trace_transaction(hash).await
                }

                /// Resolve an ENS name to an address
                async fn ens_resolve(
                    &self,
                    registry: Option<ethers::prelude::Address>,
                    ens_name: &str,
                ) -> Result<ethers::prelude::Address, crate::error::RpcError> {
                    [<$network Middleware>]::as_middleware(self).ens_resolve(registry, ens_name).await
                }

                /// Look up the ENS name associated with an address
                async fn ens_lookup(
                    &self,
                    registry: Option<ethers::prelude::Address>,
                    address: ethers::prelude::Address,
                ) -> Result<String, crate::error::RpcError> {
                    [<$network Middleware>]::as_middleware(self).ens_lookup(registry, address).await
                }

                /// Sign a transaction, if a signer is available
                async fn sign_transaction(
                    &self,
                    tx: &<$network as crate::networks::Network>::TransactionRequest,
                    from: Address,
                ) -> Result<ethers::prelude::Signature, crate::error::RpcError> {
                    [<$network Middleware>]::as_middleware(self).sign_transaction(tx, from).await
                }

                /// Sends the transaction to the entire Ethereum network and returns the
                /// transaction's hash. This will consume gas from the account that signed
                /// the transaction.
                async fn send_transaction(
                    &self,
                    tx: &<$network as crate::networks::Network>::TransactionRequest,
                    block: Option<ethers::prelude::BlockNumber>,
                ) -> Result<crate::watchers::pending_transaction::PendingTransaction<'_, $network>, crate::error::RpcError> {
                    crate::middleware::Middleware::send_transaction([<$network Middleware>]::as_middleware(self), tx, block).await
                }

                /// Send a transaction with a simple escalation policy.
                ///
                /// `policy` should be a boxed function that maps `original_gas_price`
                /// and `number_of_previous_escalations` -> `new_gas_price`.
                ///
                /// e.g. `Box::new(|start, escalation_index| start * 1250.pow(escalations) /
                /// 1000.pow(escalations))`
                async fn send_escalating<'a>(
                    &'a self,
                    tx: &<$network as crate::networks::Network>::TransactionRequest,
                    escalations: usize,
                    policy: ethers::prelude::EscalationPolicy,
                ) -> Result<crate::watchers::pending_escalator::EscalatingPending<'_, $network>, crate::error::RpcError> {
                    [<$network Middleware>]::as_middleware(self).send_escalating(tx, escalations, policy).await
                }

                /// Send the raw RLP encoded transaction to the entire Ethereum network and
                /// returns the transaction's hash This will consume gas from the account
                /// that signed the transaction.
                async fn send_raw_transaction(&self, tx: ethers::prelude::Bytes) -> Result<crate::watchers::pending_transaction::PendingTransaction<'_, $network>, crate::error::RpcError> {

                    crate::middleware::Middleware::send_raw_transaction([<$network Middleware>]::as_middleware(self), tx).await
                }

                /// Create a stream that repeatedly polls a log filter
                async fn watch_new_logs(&self, filter: &ethers::prelude::Filter) -> Result<crate::watchers::filter_watcher::LogWatcher<$network>, crate::error::RpcError> {
                    [<$network Middleware>]::as_middleware(self).watch_new_logs(filter).await

                }

                /// Create a stream that repeatedly polls a new block filter
                async fn watch_new_blocks(&self) -> Result<crate::watchers::filter_watcher::NewBlockWatcher<$network>, crate::error::RpcError> {
                    [<$network Middleware>]::as_middleware(self).watch_new_blocks().await

                }

                /// Create a stream that repeatedly polls a pending transaction filter
                async fn watch_new_pending_transactions(
                    &self,
                ) -> Result<crate::watchers::filter_watcher::PendingTransactionWatcher<$network>, crate::error::RpcError> {
                    [<$network Middleware>]::as_middleware(self).watch_new_pending_transactions().await

                }
            }

            impl<T> [<$network Middleware>] for T where
                T: crate::middleware::BaseMiddleware<$network>
                + crate::middleware::GethMiddleware<$network>
                + crate::middleware::ParityMiddleware<$network>
                + crate::middleware::Middleware<$network>
                + std::fmt::Debug
                + Send
                + Sync
            {
                #[doc(hidden)]
                fn as_base_middleware(&self) -> &dyn BaseMiddleware<$network> {
                    self
                }

                #[doc(hidden)]
                fn as_geth_middleware(&self) -> &dyn GethMiddleware<$network> {
                    self
                }

                #[doc(hidden)]
                fn as_parity_middleware(&self) -> &dyn ParityMiddleware<$network> {
                    self
                }

                #[doc(hidden)]
                fn as_middleware(&self) -> &dyn Middleware<$network> {
                    self
                }
            }
        }
    };
}
