use ethers::prelude::{
    transaction::{eip2718::TypedTransaction, eip2930::AccessListWithGasUsed},
    *,
};
use serde_json::Value;

impl_rpc!("web3_clientVersion", ClientVersion, response: String);

impl_rpc!("eth_blockNumber", BlockNumber, response: U64);

impl_rpc!(
    "eth_getBlockByHash",
    GetBlockByHash,
    params: [ H256, bool ],
    response: Option<Value>,
);

impl_rpc!(
    "eth_getBlockByNumber",
    GetBlockByNumber,
    params: [ BlockNumber, bool ],
    response: Option<Value>,
);

impl_rpc!(
    "eth_getUncleCountByBlockHash",
    GetUncleCountByHash,
    param: H256,
    response: U256,
);

impl_rpc!(
    "eth_getUncleCountByBlockNumber",
    GetUncleCountByBlockNumber,
    param: BlockNumber,
    response: U256,
);

impl_rpc!(
    "eth_getUncleByBlockHashAndIndex",
    GetUncleByHashAndIndex,
    params: [ H256, U64 ],
    response: Option<Block<H256>>,
);

impl_rpc!(
    "eth_getUncleByBlockNumberAndIndex",
    GetUncleByBlockNumberAndIndex,
    params: [ BlockNumber, U64 ],
    response: Option<Block<H256>>,
);

impl_rpc!(
    "eth_getTransactionByHash",
    GetTransactionByHash,
    param: H256,
    response: Option<Transaction>,
);

impl_rpc!(
    "eth_getTransactionReceipt",
    GetTransactionReceipt,
    param: H256,
    response: Option<TransactionReceipt>,
);

impl_rpc!(
    "eth_getBlockReceipts",
    GetBlockReceipts,
    param: BlockNumber,
    response: Vec<TransactionReceipt>,
);

impl_rpc!("eth_gasPrice", GasPrice, response: U256);

impl_rpc!("eth_accounts", Accounts, response: Vec<Address>);

impl_rpc!(
    "eth_getTransactionCount",
    GetTransactionCount,
    params: [ Address, BlockNumber ],
    response: U256,
);

impl_rpc!(
    "eth_getBalance",
    GetBalance,
    params: [ Address, BlockNumber ],
    response: U256,
);

impl_rpc!("eth_chainId", ChainId, response: U256);

impl_rpc!(
    "eth_call",
    Call,
    params: [ TypedTransaction, BlockNumber ],
    response: Bytes,
);

impl_rpc!(
    "eth_estimateGas",
    EstimateGas,
    param: TypedTransaction,
    response: U256,
);

impl_rpc!(
    "eth_createAccessList",
    CreateAccessList,
    params: [ TypedTransaction, BlockNumber ],
    response: AccessListWithGasUsed,
);

impl_rpc!(
    "eth_sendTransaction",
    SendTransaction,
    param: TypedTransaction,
    response: H256,
);

impl_rpc!(
    "eth_sendRawTransaction",
    SendRawTransaction,
    param: Bytes,
    response: H256,
);

impl_rpc!(
    "eth_sign",
    Sign,
    params: [Address, Bytes],
    response: Signature,
);

impl_rpc!("eth_getLogs", GetLogs, param: Filter, response: Vec<Log>);

impl_rpc!("eth_newBlockFilter", NewBlockFilter, response: U256);

impl_rpc!(
    "eth_newPendingTransactionFilter",
    NewPendingTransactionFilter,
    response: U256,
);

impl_rpc!("eth_newFilter", NewFilter, param: Filter, response: U256);

impl_rpc!(
    "eth_uninstallFilter",
    UninstallFilter,
    param: U256,
    response: bool,
);

impl_rpc!(
    "eth_getFilterChanges",
    GetFilterChanges,
    param: U256,
    response: Vec<Value>,
);

impl_rpc!(
    "eth_getStorageAt",
    GetStorageAt,
    params: [ Address, H256, BlockNumber ],
    response: H256,
);

impl_rpc!(
    "eth_getCode",
    GetCode,
    params: [ Address, BlockNumber ],
    response: Bytes,
);

impl_rpc!(
    "eth_getProof",
    GetProof,
    params: [ Address, Vec<H256>, BlockNumber ],
    response: EIP1186ProofResponse,
);

impl_rpc!("txpool_content", TxpoolContent, response: TxpoolContent);

impl_rpc!("txpool_inspect", TxpoolInspect, response: TxpoolInspect);

impl_rpc!("txpool_status", TxpoolStatus, response: TxpoolStatus);

impl_rpc!(
    "trace_call",
    TraceCall,
    params: [ TypedTransaction, Vec<TraceType>, BlockNumber ],
    response: BlockTrace,
);

impl_rpc!(
   "trace_rawTransaction",
   TraceRawTransaction,
   params: [ Bytes, Vec<TraceType> ],
   response: BlockTrace,
);

impl_rpc!(
    "trace_replayTransaction",
    TraceReplayTransaction,
    params: [ H256, Vec<TraceType> ],
    response: BlockTrace,
);

impl_rpc!(
    "trace_replayBlockTransactions",
    TraceReplayBlockTransactions,
    params: [ BlockNumber, Vec<TraceType>],
    response: Vec<BlockTrace>,
);

impl_rpc!(
    "trace_block",
    TraceBlock,
    param: BlockNumber,
    response: Vec<Trace>,
);

impl_rpc!(
    "trace_filter",
    TraceFilter,
    param: TraceFilter,
    response: Vec<Trace>,
);

impl_rpc!(
    "trace_get",
    TraceGet,
    params: [ H256, Vec<U64> ],
    response: Trace,
);

impl_rpc!(
    "trace_transaction",
    TraceTransaction,
    param: H256,
    response: Vec<Trace>,
);

impl_rpc!("eth_unsubscribe", Unsubscribe, param: U256, response: bool);

impl_rpc!(
    "eth_feeHistory",
    FeeHistory,
    params: [ U256, BlockNumber, Vec<f64> ],
    response: FeeHistory,
);

impl_rpc!(
    "eth_subscribe",
    SubscribeHeads,
    param: String,
    response: U256,
);

impl_rpc!(
    "eth_subscribe",
    SubscribeLogs,
    params: [String, Filter],
    response: U256,
);

impl_rpc!(
    "eth_subscribe",
    SubscribeNewPendingTransactions,
    param: String,
    response: U256,
);

impl_rpc!(
    "eth_subscribe",
    SubscribeSyncing,
    param: String,
    response: U256,
);

// TODO(James): eth_subscribe manual impls
