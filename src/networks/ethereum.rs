use ethers_core::types::{
    transaction::{eip2718::TypedTransaction, eip2930::AccessList},
    Address, Bytes, NameOrAddress, Signature, TxHash, U256, U64,
};

use std::fmt::Debug;

use crate::{
    networks::{Network, Txn},
    types::Eip1559Fees,
};

#[derive(Debug)]
pub struct Ethereum;

impl Network for Ethereum {
    type TransactionRequest = TypedTransaction;
}

impl Txn for TypedTransaction {
    fn recommend_1559(&self) -> bool {
        true
    }

    fn get_1559_fees(&self) -> crate::types::Eip1559Fees {
        match self {
            TypedTransaction::Eip1559(tx) => Eip1559Fees {
                max_fee_per_gas: tx.max_fee_per_gas,
                max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
            },
            _ => Eip1559Fees::default(),
        }
    }

    fn set_1559_fees(&mut self, fees: &Eip1559Fees) {
        if let TypedTransaction::Eip1559(tx) = self {
            // don't override with none
            if fees.max_fee_per_gas.is_some() {
                tx.max_fee_per_gas = fees.max_fee_per_gas;
            }

            if fees.max_priority_fee_per_gas.is_some() {
                tx.max_priority_fee_per_gas = fees.max_priority_fee_per_gas;
            }
        }
    }

    fn from(&self) -> Option<&Address> {
        TypedTransaction::from(self)
    }

    fn set_from(&mut self, from: Address) {
        TypedTransaction::set_from(self, from)
    }

    fn to(&self) -> Option<&NameOrAddress> {
        TypedTransaction::to(self)
    }

    fn set_to<T: Into<NameOrAddress>>(&mut self, to: T) {
        TypedTransaction::set_to(self, to)
    }

    fn nonce(&self) -> Option<&U256> {
        TypedTransaction::nonce(self)
    }

    fn set_nonce<T: Into<U256>>(&mut self, nonce: T) {
        TypedTransaction::set_nonce(self, nonce)
    }

    fn value(&self) -> Option<&U256> {
        TypedTransaction::value(self)
    }

    fn set_value<T: Into<U256>>(&mut self, value: T) {
        TypedTransaction::set_value(self, value)
    }

    fn gas(&self) -> Option<&U256> {
        TypedTransaction::gas(self)
    }

    fn set_gas<T: Into<U256>>(&mut self, gas: T) {
        TypedTransaction::set_gas(self, gas)
    }

    fn gas_price(&self) -> Option<U256> {
        TypedTransaction::gas_price(self)
    }

    fn set_gas_price<T: Into<U256>>(&mut self, gas_price: T) {
        TypedTransaction::set_gas_price(self, gas_price)
    }

    fn data(&self) -> Option<&Bytes> {
        TypedTransaction::data(self)
    }

    fn access_list(&self) -> Option<&AccessList> {
        TypedTransaction::access_list(self)
    }

    fn set_access_list(&mut self, access_list: AccessList) {
        TypedTransaction::set_access_list(self, access_list)
    }

    fn set_data(&mut self, data: Bytes) {
        TypedTransaction::set_data(self, data)
    }

    fn rlp_signed<T: Into<U64>>(&self, chain_id: T, signature: &Signature) -> Bytes {
        TypedTransaction::rlp_signed(self, chain_id, signature)
    }

    fn rlp<T: Into<U64>>(&self, chain_id: T) -> Bytes {
        TypedTransaction::rlp(self, chain_id)
    }

    fn sighash<T: Into<U64>>(&self, chain_id: T) -> TxHash {
        TypedTransaction::sighash(self, chain_id)
    }
}

impl_network_middleware!(Ethereum);

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod test {
    #[tokio::test]
    async fn it_makes_a_req() {
        use super::EthereumMiddleware;
        let provider: crate::connections::Http =
            "https://mainnet.infura.io/v3/5cfdec76313b457cb696ff1b89cee7ee".parse().unwrap();
        dbg!(provider.get_block_number().await.unwrap());
    }
}
