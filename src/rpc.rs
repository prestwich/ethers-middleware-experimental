use ethers::prelude::*;
use serde_json::Value;

impl_rpc!("web3_clientVersion", ClientVersion, response: String);

impl_rpc!("eth_blockNumber", BlockNumber, response: U256);

impl_rpc!("eth_getBlockByHash", BlockByHash, params: [ H256, bool ], response: Option<Value>);

impl_rpc!("eth_getBlockByNumber", BlockByNumber, params: [ BlockNumber, bool ], response: Option<Value>);

impl_rpc!(
    "eth_getUncleCountByBlockHash",
    GetUncleCountByHash,
    param: H256,
    response: U256
);

impl_rpc!(
    "eth_getUncleCountByBlockNumber",
    GetUncleCountByBlockNumber,
    param: BlockNumber,
    response: U256
);

impl_rpc!(
    "eth_getUncleByBlockHashAndIndex",
    GetUncleByHashAndIndex,
    params: [ H256, U64 ],
    response: Option<Block<H256>>
);

impl_rpc!(
    "eth_getUncleByBlockNumberAndIndex",
    GetUncleByBlockNumberAndIndex,
    params: [ BlockNumber, U64 ],
    response: Option<Block<H256>>
);

impl_rpc!(
    "eth_getTransactionByHash",
    GetTransactionByHash,
    param: H256,
    response: Option<Transaction>
);

impl_rpc!(
    "eth_getTransactionReceipt",
    GetTransactionReceipt,
    param: H256,
    response: Option<TransactionReceipt>
);

impl_rpc!(
    "eth_getBlockReceipt",
    GetBlockReceipt,
    param: BlockNumber,
    response: Vec<TransactionReceipt>
);

impl_rpc!("eth_gasPrice", GasPrice, response: U256);
