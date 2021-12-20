//! An abstract `Network` trait used to parameterize chain-specific behavior

pub mod ethereum;
pub use ethereum::{Ethereum, EthereumMiddleware, EthereumPubSubMiddleware};

use ethers_core::types::{
    transaction::eip2930::AccessList, Address, Bytes, NameOrAddress, Signature, H256, U256, U64,
};

use crate::types::Eip1559Fees;

use std::fmt::Debug;

pub trait Txn: Default + serde::Serialize + Debug + Clone + Send + Sync {
    fn recommend_1559(&self) -> bool {
        false
    }

    fn get_1559_fees(&self) -> Eip1559Fees;
    fn set_1559_fees(&mut self, fees: &Eip1559Fees);

    fn from(&self) -> Option<&Address>;
    fn set_from(&mut self, from: Address);

    fn to(&self) -> Option<&NameOrAddress>;
    fn set_to<T: Into<NameOrAddress>>(&mut self, to: T);

    fn nonce(&self) -> Option<&U256>;
    fn set_nonce<T: Into<U256>>(&mut self, nonce: T);

    fn value(&self) -> Option<&U256>;
    fn set_value<T: Into<U256>>(&mut self, value: T);

    fn gas(&self) -> Option<&U256>;
    fn set_gas<T: Into<U256>>(&mut self, gas: T);

    fn gas_price(&self) -> Option<U256>;
    fn set_gas_price<T: Into<U256>>(&mut self, gas_price: T);

    fn data(&self) -> Option<&Bytes>;
    fn set_data(&mut self, data: Bytes);

    fn access_list(&self) -> Option<&AccessList>;
    fn set_access_list(&mut self, access_list: AccessList);

    fn rlp_signed<T: Into<U64>>(&self, chain_id: T, signature: &Signature) -> Bytes;
    fn rlp<T: Into<U64>>(&self, chain_id: T) -> Bytes;
    fn sighash<T: Into<U64>>(&self, chain_id: T) -> H256;
}

pub trait Network: Send + Sync + Debug {
    type TransactionRequest: Txn;
}
