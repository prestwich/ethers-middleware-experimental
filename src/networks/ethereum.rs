use ethers::prelude::{
    transaction::{eip2718::TypedTransaction, eip2930::AccessList},
    Address, Bytes, NameOrAddress, Signature, TxHash, U256, U64,
};

use std::fmt::Debug;

use crate::{
    middleware::{BaseMiddleware, GethMiddleware, Middleware, ParityMiddleware},
    networks::{Network, Txn},
};

#[derive(Debug)]
pub struct Ethereum;

impl Network for Ethereum {
    type TransactionRequest = TypedTransaction;
}

impl Txn for TypedTransaction {
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

mod test {
    use super::EthereumMiddleware;

    #[tokio::test]
    async fn it_makes_a_req() {
        let provider: crate::connections::http::Http =
            "https://mainnet.infura.io/v3/5cfdec76313b457cb696ff1b89cee7ee"
                .parse()
                .unwrap();
        dbg!(provider.get_block_number().await.unwrap());
    }
}
