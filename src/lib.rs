#[macro_use]
pub mod macros;
pub mod connections;
pub mod ens;
pub mod error;
pub mod middleware;
pub mod networks;
pub mod rpc;
pub mod subscriptions;
pub mod types;
pub mod watchers;

// We re-export the defaults
pub use connections::{Http, MockRpcConnection, QuorumProvider, RetryingProvider, Ws};

#[cfg(not(target_arch = "wasm32"))]
pub use connections::Ipc;

pub use error::RpcError;

#[cfg(not(feature = "celo"))]
pub use networks::ethereum::{
    DefaultNetwork, EscalatingPending, LogStream, LogWatcher, Middleware, NewBlockStream,
    NewBlockWatcher, PendingTransaction, PendingTransactionStream, PendingTransactionWatcher,
    PubSubMiddleware, SyncingStream, TransactionStream,
};

// feature-enabled support for dev-rpc methods
#[cfg(feature = "dev-rpc")]
pub use middleware::dev_rpc::DevRpcMiddleware;

#[cfg(all(test, not(feature = "celo")))]
mod test {
    use super::*;
    use ethers_core::utils::Ganache;
    #[tokio::test]
    async fn compile_check() {
        let ganache = Ganache::new().block_time(2u64).spawn();
        let provider: Http = ganache.endpoint().parse().unwrap();
        let b = provider.get_block_number().await.unwrap();
        dbg!(&b);
    }
}

// TODO: REMOVE FROM HERE

pub type EscalationPolicy =
    Box<dyn Fn(ethers_core::types::U256, usize) -> ethers_core::types::U256 + Send + Sync>;

use futures_core::stream::Stream;
use futures_util::{stream::StreamExt, FutureExt};
use std::{future::Future, pin::Pin, time::Duration};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use futures_timer::Delay;
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_timer::Delay;

pub(crate) const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(10);
// https://github.com/tomusdrw/rust-web3/blob/befcb2fb8f3ca0a43e3081f68886fa327e64c8e6/src/api/eth_filter.rs#L20
pub(crate) fn interval(duration: Duration) -> impl Stream<Item = ()> + Send + Unpin {
    futures_util::stream::unfold((), move |_| Delay::new(duration).map(|_| Some(((), ()))))
        .map(drop)
}

#[cfg(target_arch = "wasm32")]
pub(crate) type PinBoxFut<'a, T> = Pin<Box<dyn Future<Output = Result<T, RpcError>> + 'a>>;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type PinBoxFut<'a, T> = Pin<Box<dyn Future<Output = Result<T, RpcError>> + Send + 'a>>;
// TODO: REMOVE TO HERE
