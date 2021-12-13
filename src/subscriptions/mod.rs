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
    middleware::PubSubMiddleware,
    networks::Network,
    types::{Notification, SyncData},
};

pub type NewBlockStream<'a, N> = SubscriptionStream<'a, Block<TxHash>, N>;
pub type LogStream<'a, N> = SubscriptionStream<'a, Log, N>;
pub type PendingTransactionStream<'a, N> = SubscriptionStream<'a, TxHash, N>;
pub type SyncingStream<'a, N> = SubscriptionStream<'a, SyncData, N>;

#[must_use = "subscriptions do nothing unless you stream them"]
#[pin_project(PinnedDrop)]
pub struct SubscriptionStream<'a, R, N>
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

impl<'a, R, N> SubscriptionStream<'a, R, N>
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
    /// using [`Provider::subscribe`] instead of `SubscriptionStream::new`.
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
impl<'a, R, N> Stream for SubscriptionStream<'a, R, N>
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
impl<R, N> PinnedDrop for SubscriptionStream<'_, R, N>
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
