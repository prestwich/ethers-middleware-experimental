//! An abstract `Network` trait used to parameterize chain-specific behavior

pub mod ethereum;
pub use ethereum::{Ethereum, EthereumMiddleware, EthereumPubSubMiddleware};

use ethers_core::types::{
    transaction::eip2930::AccessList, Address, Bytes, NameOrAddress, Signature, H256, U256, U64,
};

use crate::types::Eip1559Fees;

use std::fmt::Debug;

/// A transaction request. This type abstracts the body of the
///
/// Implementors may not add behavior to all setters/getters, e.g. for
/// networks that do not support EIP-1559 yet, the `set_1559_fees` method may
/// have no effect.
pub trait TransactionRequest: Default + serde::Serialize + Debug + Clone + Send + Sync {
    /// True if the transaction type prefers 1559-style fees.
    ///
    /// This should be overridden by implementors.
    fn recommend_1559(&self) -> bool {
        false
    }

    /// Getter for `1559_fees`
    fn get_1559_fees(&self) -> Eip1559Fees;
    /// Setter for `1559_fees`
    fn set_1559_fees(&mut self, fees: &Eip1559Fees);

    /// Getter for `from`
    fn from(&self) -> Option<&Address>;
    /// Setter for `from`
    fn set_from(&mut self, from: Address);

    /// Getter for `to`
    fn to(&self) -> Option<&NameOrAddress>;
    /// Setter for `to`
    fn set_to<T: Into<NameOrAddress>>(&mut self, to: T);

    /// Getter for `nonce`
    fn nonce(&self) -> Option<&U256>;
    /// Setter for `nonce`
    fn set_nonce<T: Into<U256>>(&mut self, nonce: T);

    /// Getter for `value`
    fn value(&self) -> Option<&U256>;
    /// Setter for `value`
    fn set_value<T: Into<U256>>(&mut self, value: T);

    /// Getter for `gas`
    fn gas(&self) -> Option<&U256>;
    /// Setter for `gas`
    fn set_gas<T: Into<U256>>(&mut self, gas: T);

    /// Getter for `gas_price`
    fn gas_price(&self) -> Option<U256>;
    /// Setter for `gas_price`
    fn set_gas_price<T: Into<U256>>(&mut self, gas_price: T);

    /// Getter for `data`
    fn data(&self) -> Option<&Bytes>;
    /// Setter for `data`
    fn set_data(&mut self, data: Bytes);

    /// Getter for `access_list`
    fn access_list(&self) -> Option<&AccessList>;
    /// Setter for `access_list`
    fn set_access_list(&mut self, access_list: AccessList);

    /// RLP-serialize with a chain id and signature
    fn rlp_signed<T: Into<U64>>(&self, chain_id: T, signature: &Signature) -> Bytes;
    /// RLP-serialize the unsigned transaction
    fn rlp<T: Into<U64>>(&self, chain_id: T) -> Bytes;
    /// Return the hash which a signer ought to sign
    fn sighash<T: Into<U64>>(&self, chain_id: T) -> H256;
}

pub trait Network: Send + Sync + Debug {
    type TransactionRequest: TransactionRequest;
}
