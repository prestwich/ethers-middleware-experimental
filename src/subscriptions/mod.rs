//! PubSubMiddleware-based subscriptions

use std::{
    marker::PhantomData,
    pin::Pin,
    task::{self, Poll},
};

use ethers_core::types::{Block, Log, TxHash, U256};
use futures_channel::mpsc::UnboundedReceiver;
use futures_core::Stream;
use pin_project::{pin_project, pinned_drop};
use serde::de::DeserializeOwned;

use crate::{
    error::RpcError,
    middleware::{Middleware, PubSubMiddleware},
    networks::Network,
    types::{Notification, SyncData},
    watchers::filter_watcher::GenericTransactionStream,
};

/// A new block stream for some network
pub type GenericNewBlockStream<'a, N> = GenericSubscriptionStream<'a, Block<TxHash>, N>;
/// A log stream for some network
pub type GenericLogStream<'a, N> = GenericSubscriptionStream<'a, Log, N>;
/// A pending transaction stream for some network
pub type GenericPendingTransactionStream<'a, N> = GenericSubscriptionStream<'a, TxHash, N>;
/// A syncing stream for some network
pub type GenericSyncingStream<'a, N> = GenericSubscriptionStream<'a, SyncData, N>;

/// A stream emitting the output of a subscription
#[must_use = "subscriptions do nothing unless you stream them"]
#[pin_project(PinnedDrop)]
pub struct GenericSubscriptionStream<'a, R, N>
where
    R: DeserializeOwned,
    N: Network,
{
    /// The subscription's installed id on the ethereum node
    pub id: U256,
    provider: &'a dyn PubSubMiddleware<N>,
    #[pin]
    rx: UnboundedReceiver<Notification>,
    ret: PhantomData<R>,
}

impl<'a, R, N> GenericSubscriptionStream<'a, R, N>
where
    R: DeserializeOwned,
    N: Network,
{
    /// Creates a new subscription stream for the provided subscription id.
    ///
    /// ### Note
    /// Most providers treat `SubscriptionStream` IDs as global singletons.
    /// Instantiating this directly with a known ID will likely cause any
    /// existing streams with that ID to end. To avoid this, start a new stream
    /// using the [`PubSubMiddleware`] subscribe functions instead of
    /// `SubscriptionStream::new`.
    pub fn new(id: U256, provider: &'a dyn PubSubMiddleware<N>) -> Result<Self, RpcError> {
        // Call the underlying PubsubClient's subscribe
        let rx = provider.pubsub_provider().install_listener(id)?;
        Ok(Self {
            id,
            provider,
            rx,
            ret: PhantomData,
        })
    }

    /// Unsubscribes from the subscription.
    pub async fn uninstall(&self) -> Result<(), RpcError> {
        self.provider.pubsub_provider().uninstall_listener(self.id)
    }
}

// Each subscription item is a serde_json::Value which must be decoded to the
// subscription's return type.
// TODO: Can this be replaced with an `rx.map` in the constructor?
impl<'a, R, N> Stream for GenericSubscriptionStream<'a, R, N>
where
    R: DeserializeOwned,
    N: Network,
{
    type Item = R;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut task::Context) -> Poll<Option<Self::Item>> {
        let this = self.project();
        match futures_util::ready!(this.rx.poll_next(ctx)) {
            Some(item) => match serde_json::from_value(item.result) {
                Ok(res) => Poll::Ready(Some(res)),
                _ => Poll::Pending,
            },
            None => Poll::Ready(None),
        }
    }
}

#[pinned_drop]
impl<R, N> PinnedDrop for GenericSubscriptionStream<'_, R, N>
where
    R: DeserializeOwned,
    N: Network,
{
    fn drop(self: Pin<&mut Self>) {
        // on drop it removes the handler from the websocket so that it stops
        // getting populated. We need to call `unsubscribe` explicitly to cancel
        // the subscription
        let _ = self.provider.pubsub_provider().uninstall_listener(self.id);
    }
}

impl<'a, N> GenericSubscriptionStream<'a, TxHash, N>
where
    N: Network,
{
    /// Returns a stream that yields the `Transaction`s for the transaction hashes this stream
    /// yields.
    ///
    /// This internally calls `Provider::get_transaction` with every new transaction.
    /// No more than n futures will be buffered at any point in time, and less than n may also be
    /// buffered depending on the state of each future.
    pub fn transactions_unordered(self, n: usize) -> GenericTransactionStream<'a, Self, N> {
        let provider = self.provider.as_middleware();
        let provider = Middleware::as_base_middleware(provider);
        GenericTransactionStream::new(provider, self, n)
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::{Http, Middleware, PubSubMiddleware, TransactionStream, Ws};
    use ethers_core::{
        types::{
            transaction::eip2718::TypedTransaction, Transaction, TransactionReceipt,
            TransactionRequest,
        },
        utils::{Ganache, Geth},
    };
    use futures_util::{stream, FutureExt, StreamExt};
    use std::collections::HashSet;

    #[tokio::test]
    async fn can_stream_pending_transactions() {
        let num_txs = 5;
        let geth = Geth::new().block_time(2u64).spawn();
        let provider: Http = geth.endpoint().parse::<Http>().unwrap();
        let ws_provider = Ws::connect(geth.ws_endpoint()).await.unwrap();

        let accounts = provider.accounts().await.unwrap();
        let tx: TypedTransaction = TransactionRequest::new()
            .from(accounts[0])
            .to(accounts[0])
            .value(1e18 as u64)
            .into();

        let mut sending = futures_util::future::join_all((0..num_txs).map(|_| async {
            provider
                .send_transaction(&tx.clone(), None)
                .await
                .unwrap()
                .await
                .unwrap()
                .unwrap()
        }))
        .fuse();

        let mut watch_tx_stream = provider
            .watch_new_pending_transactions()
            .await
            .unwrap()
            .transactions_unordered(num_txs)
            .fuse();

        let mut sub_tx_stream = ws_provider
            .stream_new_pending_transactions()
            .await
            .unwrap()
            .transactions_unordered(2)
            .fuse();

        let mut sent: Option<Vec<TransactionReceipt>> = None;
        let mut watch_received: Vec<Transaction> = Vec::with_capacity(num_txs);
        let mut sub_received: Vec<Transaction> = Vec::with_capacity(num_txs);

        loop {
            futures_util::select! {
                txs = sending => {
                    sent = Some(txs)
                },
                tx = watch_tx_stream.next() => watch_received.push(tx.unwrap().unwrap()),
                tx = sub_tx_stream.next() => sub_received.push(tx.unwrap().unwrap()),
            };
            if watch_received.len() == num_txs && sub_received.len() == num_txs {
                if let Some(ref sent) = sent {
                    assert_eq!(sent.len(), watch_received.len());
                    let sent_txs = sent
                        .iter()
                        .map(|tx| tx.transaction_hash)
                        .collect::<HashSet<_>>();
                    assert_eq!(sent_txs, watch_received.iter().map(|tx| tx.hash).collect());
                    assert_eq!(sent_txs, sub_received.iter().map(|tx| tx.hash).collect());
                    break;
                }
            }
        }
    }

    #[tokio::test]
    async fn can_stream_transactions() {
        let ganache = Ganache::new().block_time(2u64).spawn();
        let provider: Http = ganache.endpoint().parse().unwrap();
        // .with_sender(ganache.addresses()[0]);

        let accounts = provider.accounts().await.unwrap();

        let tx: TypedTransaction = TransactionRequest::new()
            .from(accounts[0])
            .to(accounts[0])
            .value(1e18 as u64)
            .into();

        let txs = futures_util::future::join_all((0..3).map(|_| async {
            provider
                .send_transaction(&tx.clone(), None)
                .await
                .unwrap()
                .await
                .unwrap()
        }))
        .await;

        let stream = TransactionStream::<'_, _>::new(
            &provider,
            stream::iter(txs.iter().cloned().map(|tx| tx.unwrap().transaction_hash)),
            10,
        );
        let res = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(res.len(), txs.len());
        assert_eq!(
            res.into_iter().map(|tx| tx.hash).collect::<HashSet<_>>(),
            txs.into_iter()
                .map(|tx| tx.unwrap().transaction_hash)
                .collect()
        );
    }
}
