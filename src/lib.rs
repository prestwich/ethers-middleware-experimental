#[macro_use]
pub mod macros;

pub mod error;
pub mod middleware;
pub mod network;
pub mod provider;
pub mod rpc;
pub mod transports;
pub mod types;

pub mod filter_watcher;
pub mod pending_transaction;

// TODO: REMOVE FROM HERE

use error::RpcError;
use futures_core::stream::Stream;
use futures_util::{stream::StreamExt, FutureExt};
use std::{future::Future, pin::Pin, time::Duration};

#[cfg(not(target_arch = "wasm32"))]
pub use futures_timer::Delay;
#[cfg(target_arch = "wasm32")]
pub use wasm_timer::Delay;

pub(crate) const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(10);
// https://github.com/tomusdrw/rust-web3/blob/befcb2fb8f3ca0a43e3081f68886fa327e64c8e6/src/api/eth_filter.rs#L20
pub(crate) fn interval(duration: Duration) -> impl Stream<Item = ()> + Send + Unpin {
    futures_util::stream::unfold((), move |_| Delay::new(duration).map(|_| Some(((), ()))))
        .map(drop)
}

#[cfg(target_arch = "wasm32")]
pub(crate) type PinBoxFut<'a, T> = Pin<Box<dyn Future<Output = Result<T, ProviderError>> + 'a>>;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type PinBoxFut<'a, T> = Pin<Box<dyn Future<Output = Result<T, RpcError>> + Send + 'a>>;
// TODO: REMOVE TO HERE
