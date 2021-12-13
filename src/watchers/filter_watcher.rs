use ethers_core::types::{Log, Transaction, TxHash, H256, U256};
use futures_core::{stream::Stream, Future};
use futures_util::{stream::FuturesUnordered, FutureExt, StreamExt};
use pin_project::pin_project;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::VecDeque,
    fmt::Debug,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
    vec::IntoIter,
};

use crate::{
    error::RpcError, interval, middleware::BaseMiddleware, networks::Network, PinBoxFut,
    DEFAULT_POLL_INTERVAL,
};

enum FilterWatcherState<'a, R> {
    WaitForInterval,
    GetFilterChanges(PinBoxFut<'a, Vec<serde_json::Value>>),
    NextItem(IntoIter<R>),
}

pub type NewBlockWatcher<'a, N> = FilterWatcher<'a, H256, N>;
pub type PendingTransactionWatcher<'a, N> = FilterWatcher<'a, TxHash, N>;
pub type LogWatcher<'a, N> = FilterWatcher<'a, Log, N>;

#[must_use = "filters do nothing unless you stream them"]
#[pin_project]
/// Streams data from an installed filter via `eth_getFilterChanges`
pub struct FilterWatcher<'a, R, N> {
    /// The filter's installed id on the ethereum node
    pub id: U256,

    provider: &'a dyn BaseMiddleware<N>,

    // The polling interval
    interval: Box<dyn Stream<Item = ()> + Send + Unpin>,

    state: FilterWatcherState<'a, R>,
}

impl<'a, R, N> FilterWatcher<'a, R, N>
where
    R: Send + Sync + DeserializeOwned,
{
    /// Creates a new watcher with the provided factory and filter id.
    pub fn new<T: Into<U256>>(id: T, provider: &'a dyn BaseMiddleware<N>) -> Self {
        Self {
            id: id.into(),
            interval: Box::new(interval(DEFAULT_POLL_INTERVAL)),
            state: FilterWatcherState::WaitForInterval,
            provider,
        }
    }

    /// Sets the stream's polling interval
    pub fn interval(mut self, duration: Duration) -> Self {
        self.interval = Box::new(interval(duration));
        self
    }

    /// Alias for Box::pin, must be called in order to pin the stream and be able
    /// to call `next` on it.
    pub fn stream(self) -> Pin<Box<Self>> {
        Box::pin(self)
    }
}

// Pattern for flattening the returned Vec of filter changes taken from
// https://github.com/tomusdrw/rust-web3/blob/f043b222744580bf4be043da757ab0b300c3b2da/src/api/eth_filter.rs#L50-L67
impl<'a, R, N> Stream for FilterWatcher<'a, R, N>
where
    R: Serialize + Send + Sync + DeserializeOwned + Debug + 'a,
    N: Network,
{
    type Item = R;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let id = *this.id;

        *this.state = match this.state {
            FilterWatcherState::WaitForInterval => {
                // Wait the polling period
                let _ready = futures_util::ready!(this.interval.poll_next_unpin(cx));

                // create a new instance of the future
                cx.waker().wake_by_ref();
                let fut = Box::pin(this.provider.get_filter_changes(id));
                FilterWatcherState::GetFilterChanges(fut)
            }
            FilterWatcherState::GetFilterChanges(fut) => {
                // NOTE: If the provider returns an error, this will return an empty
                // vector. Should we make this return a Result instead? Ideally if we're
                // in a streamed loop we wouldn't want the loop to terminate if an error
                // is encountered (since it might be a temporary error).
                let items: Vec<R> = futures_util::ready!(fut.as_mut().poll(cx))
                    .unwrap_or_default()
                    .into_iter()
                    .map(serde_json::from_value::<R>)
                    .collect::<Result<_, _>>()
                    .unwrap_or_default();
                cx.waker().wake_by_ref();
                FilterWatcherState::NextItem(items.into_iter())
            }
            // Consume 1 element from the vector. If more elements are in the vector,
            // the next call will immediately go to this branch instead of trying to get
            // filter changes again. Once the whole vector is consumed, it will poll again
            // for new logs
            FilterWatcherState::NextItem(iter) => {
                cx.waker().wake_by_ref();
                match iter.next() {
                    Some(item) => return Poll::Ready(Some(item)),
                    None => FilterWatcherState::WaitForInterval,
                }
            }
        };

        Poll::Pending
    }
}

impl<'a, N> PendingTransactionWatcher<'a, N>
where
    N: Network,
{
    /// Returns a stream that yields the `Transaction`s for the transaction hashes this stream
    /// yields.
    ///
    /// This internally calls `Provider::get_transaction` with every new transaction.
    /// No more than n futures will be buffered at any point in time, and less than n may also be
    /// buffered depending on the state of each future.
    pub fn transactions_unordered(self, n: usize) -> TransactionStream<'a, Self, N> {
        TransactionStream::new(self.provider, self, n)
    }
}

/// Errors `TransactionStream` can throw
#[derive(Debug, thiserror::Error)]
pub enum GetTransactionError {
    #[error("Failed to get transaction `{0}`: {1}")]
    RpcError(TxHash, RpcError),
    /// `get_transaction` resulted in a `None`
    #[error("Transaction `{0}` not found")]
    NotFound(TxHash),
}

impl From<GetTransactionError> for RpcError {
    fn from(err: GetTransactionError) -> Self {
        match err {
            GetTransactionError::RpcError(_, err) => err,
            err @ GetTransactionError::NotFound(_) => RpcError::CustomError(err.to_string()),
        }
    }
}

type TransactionFut<'a> = Pin<Box<dyn Future<Output = TransactionResult> + 'a>>;

type TransactionResult = Result<Transaction, GetTransactionError>;

/// Drains a stream of transaction hashes and yields entire `Transaction`.
#[must_use = "streams do nothing unless polled"]
pub struct TransactionStream<'a, St, N>
where
    N: Network,
{
    /// Currently running futures pending completion.
    pending: FuturesUnordered<TransactionFut<'a>>,
    /// Temporary buffered transaction that get started as soon as another future finishes.
    buffered: VecDeque<TxHash>,
    /// The provider that gets the transaction
    provider: &'a dyn BaseMiddleware<N>,
    /// A stream of transaction hashes.
    stream: St,
    /// max allowed futures to execute at once.
    max_concurrent: usize,
}

impl<'a, St, N> TransactionStream<'a, St, N>
where
    N: Network,
{
    /// Create a new `TransactionStream` instance
    pub fn new(provider: &'a dyn BaseMiddleware<N>, stream: St, max_concurrent: usize) -> Self {
        Self {
            pending: Default::default(),
            buffered: Default::default(),
            provider,
            stream,
            max_concurrent,
        }
    }

    /// Push a future into the set
    fn push_tx(&mut self, tx: TxHash) {
        let fut = self
            .provider
            .get_transaction(tx)
            .then(move |res| match res {
                Ok(Some(tx)) => futures_util::future::ok(tx),
                Ok(None) => futures_util::future::err(GetTransactionError::NotFound(tx)),
                Err(err) => futures_util::future::err(GetTransactionError::RpcError(tx, err)),
            });
        self.pending.push(Box::pin(fut));
    }
}

impl<'a, St, N> Stream for TransactionStream<'a, St, N>
where
    St: Stream<Item = TxHash> + Unpin + 'a,
    N: Network,
{
    type Item = TransactionResult;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // drain buffered transactions first
        while this.pending.len() < this.max_concurrent {
            if let Some(tx) = this.buffered.pop_front() {
                this.push_tx(tx);
            } else {
                break;
            }
        }

        let mut stream_done = false;
        loop {
            match Stream::poll_next(Pin::new(&mut this.stream), cx) {
                Poll::Ready(Some(tx)) => {
                    if this.pending.len() < this.max_concurrent {
                        this.push_tx(tx);
                    } else {
                        this.buffered.push_back(tx);
                    }
                }
                Poll::Ready(None) => {
                    stream_done = true;
                    break;
                }
                _ => break,
            }
        }

        // poll running futures
        if let tx @ Poll::Ready(Some(_)) = this.pending.poll_next_unpin(cx) {
            return tx;
        }

        if stream_done && this.pending.is_empty() {
            // all done
            return Poll::Ready(None);
        }

        Poll::Pending
    }
}
