use std::{
    collections::HashMap,
    fmt::Debug,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use async_trait::async_trait;
use ethers_core::types::{U256, U64};
use futures_channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures_util::{future::join_all, stream::select_all, FutureExt, StreamExt};

use crate::{
    connections::RpcConnection,
    error::RpcError,
    types::{Notification, RawRequest, RawResponse},
};

if_wasm! {
    use wasm_bindgen_futures::spawn_local;
}

use super::PubSubConnection;

#[derive(Debug, Clone)]
pub struct QuorumProvider<P> {
    /// What kind of quorum is required
    quorum: Quorum,
    /// The weight at which quorum is reached
    quorum_weight: u64,
    /// All the internal providers this providers runs
    providers: Vec<WeightedProvider<P>>,
}

impl<P> QuorumProvider<P> {
    /// Convenience method for creating a `QuorumProviderBuilder` with same `JsonRpcClient` types
    pub fn builder() -> QuorumProviderBuilder<P> {
        QuorumProviderBuilder::default()
    }

    pub fn new(quorum: Quorum, providers: impl IntoIterator<Item = WeightedProvider<P>>) -> Self {
        Self::builder()
            .add_providers(providers)
            .quorum(quorum)
            .build()
    }

    pub fn providers(&self) -> &[WeightedProvider<P>] {
        &self.providers
    }

    /// The weight at which the provider reached a quorum
    pub fn quorum_weight(&self) -> u64 {
        self.quorum_weight
    }

    pub fn add_provider(&mut self, provider: WeightedProvider<P>) {
        self.providers.push(provider);
        self.quorum_weight = self.quorum.weight(&self.providers)
    }
}

impl QuorumProvider<Box<dyn RpcConnection>> {
    /// Create a `QuorumProvider` for different `RpcConnection` types
    pub fn dyn_rpc() -> QuorumProviderBuilder<Box<dyn RpcConnection>> {
        Self::builder()
    }
}

impl QuorumProvider<Box<dyn PubSubConnection>> {
    /// Create a `QuorumProvider` for different `PubSubConnection` types
    pub fn dyn_pub_sub() -> QuorumProviderBuilder<Box<dyn PubSubConnection>> {
        Self::builder()
    }
}

#[derive(Debug, Clone)]
pub struct QuorumProviderBuilder<P> {
    quorum: Quorum,
    providers: Vec<WeightedProvider<P>>,
}

impl<P> Default for QuorumProviderBuilder<P> {
    fn default() -> Self {
        Self {
            quorum: Default::default(),
            providers: Vec::new(),
        }
    }
}

impl<P> QuorumProviderBuilder<P> {
    pub fn add_provider(mut self, provider: WeightedProvider<P>) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn add_providers(
        mut self,
        providers: impl IntoIterator<Item = WeightedProvider<P>>,
    ) -> Self {
        for provider in providers {
            self.providers.push(provider);
        }
        self
    }

    /// Set the kind of quorum
    pub fn quorum(mut self, quorum: Quorum) -> Self {
        self.quorum = quorum;
        self
    }

    pub fn build(self) -> QuorumProvider<P> {
        let quorum_weight = self.quorum.weight(&self.providers);
        QuorumProvider {
            quorum: self.quorum,
            quorum_weight,
            providers: self.providers,
        }
    }
}

impl<P> QuorumProvider<P>
where
    P: RpcConnection,
{
    /// Returns the block height that _all_ providers have surpassed.
    ///
    /// This is the minimum of all provider's block numbers
    async fn get_minimum_block_number(&self) -> Result<U64, RpcError> {
        let mut numbers = join_all(self.providers.iter().map(|provider| async move {
            let block = provider
                .inner
                ._request(RawRequest::new("eth_blockNumber", ())?)
                .await?
                .into_result()?;
            serde_json::from_value::<U64>(block).map_err(RpcError::from)
        }))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
        numbers.sort();

        numbers
            .into_iter()
            .next()
            .ok_or_else(|| RpcError::CustomError("No Providers".to_string()))
    }

    /// Normalizes the request payload depending on the call
    async fn normalize_request(&self, request: &mut RawRequest) {
        match request.method() {
            "eth_call"
            | "eth_createAccessList"
            | "eth_getStorageAt"
            | "eth_getCode"
            | "eth_getProof"
            | "trace_call"
            | "trace_block" => {
                // calls that include the block number in the params at the last index of json array
                if let Some(block) = request
                    .mut_params()
                    .as_array_mut()
                    .and_then(|arr| arr.last_mut())
                {
                    if Some("latest") == block.as_str() {
                        // replace `latest` with the minimum block height of all providers
                        if let Ok(minimum) = self
                            .get_minimum_block_number()
                            .await
                            .and_then(|num| Ok(serde_json::to_value(num)?))
                        {
                            *block = minimum
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl<P> PubSubConnection for QuorumProvider<P>
where
    P: PubSubConnection,
{
    fn uninstall_listener(&self, id: U256) -> Result<(), RpcError> {
        for provider in self.providers.iter() {
            provider.inner.uninstall_listener(id)?;
        }
        Ok(())
    }

    fn install_listener(&self, id: U256) -> Result<UnboundedReceiver<Notification>, RpcError> {
        let mut notifications = Vec::with_capacity(self.providers.len());
        for provider in &self.providers {
            let weight = provider.weight;
            let fut = provider
                .inner
                .install_listener(id)?
                .map(move |val| (val, weight));
            notifications.push(Box::pin(fut) as WeightedNotificationStream);
        }
        let (tx, rx) = mpsc::unbounded();
        QuorumStream::new(tx, self.quorum_weight, notifications).spawn();
        Ok(rx)
    }
}

/// Determines when the provider reached a quorum
#[derive(Debug, Copy, Clone)]
pub enum Quorum {
    ///  The quorum is reached when all providers return the exact value
    All,
    /// The quorum is reached when the majority of the providers have returned a
    /// matching value, taking into account their weight.
    Majority,
    /// The quorum is reached when the cumulative weight of a matching return
    /// exceeds the given percentage of the total weight.
    ///
    /// NOTE: this must be less than `100u8`
    Percentage(u8),
    /// The quorum is reached when the given number of provider agree
    /// The configured weight is ignored in this case.
    ProviderCount(usize),
    /// The quorum is reached once the accumulated weight of the matching return
    /// exceeds this weight.
    Weight(u64),
}

impl Quorum {
    fn weight<P>(self, providers: &[WeightedProvider<P>]) -> u64 {
        match self {
            Quorum::All => providers.iter().map(|p| p.weight).sum::<u64>(),
            Quorum::Majority => {
                let total = providers.iter().map(|p| p.weight).sum::<u64>();
                let rem = total % 2;
                total / 2 + rem
            }
            Quorum::Percentage(p) => {
                providers.iter().map(|p| p.weight).sum::<u64>() * (p as u64) / 100
            }
            Quorum::ProviderCount(num) => {
                // take the lowest `num` weights
                let mut weights = providers.iter().map(|p| p.weight).collect::<Vec<_>>();
                weights.sort_unstable();
                weights.into_iter().take(num).sum()
            }
            Quorum::Weight(w) => w,
        }
    }
}

impl Default for Quorum {
    fn default() -> Self {
        Quorum::Majority
    }
}

// A future that returns the provider's response and it's index within the
// `QuorumProvider` provider set
#[cfg(target_arch = "wasm32")]
type PendingRequest<'a> =
    Pin<Box<dyn Future<Output = (Result<RawResponse, RpcError>, usize)> + 'a>>;
#[cfg(not(target_arch = "wasm32"))]
type PendingRequest<'a> =
    Pin<Box<dyn Future<Output = (Result<RawResponse, RpcError>, usize)> + 'a + Send>>;

/// A future that only returns a value of the `QuorumProvider`'s provider
/// reached a quorum.
struct QuorumRequest<'a, P> {
    inner: &'a QuorumProvider<P>,
    /// The different answers with their cumulative weight
    responses: Vec<(RawResponse, u64)>,
    /// All the errors the provider yielded
    errors: Vec<RpcError>,
    // Requests currently pending
    requests: Vec<PendingRequest<'a>>,
}

impl<'a, P> QuorumRequest<'a, P> {
    fn new(inner: &'a QuorumProvider<P>, requests: Vec<PendingRequest<'a>>) -> Self {
        Self {
            responses: Vec::new(),
            errors: Vec::new(),
            inner,
            requests,
        }
    }
}

impl<'a, P> Future for QuorumRequest<'a, P> {
    type Output = Result<RawResponse, RpcError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        for n in (0..this.requests.len()).rev() {
            let mut request = this.requests.swap_remove(n);
            match request.poll_unpin(cx) {
                Poll::Ready((Ok(val), idx)) => {
                    let response_weight = this.inner.providers[idx].weight;
                    if let Some((_, weight)) = this.responses.iter_mut().find(|(v, _)| val == *v) {
                        // add the weight to equal response value
                        *weight += response_weight;
                        if *weight >= this.inner.quorum_weight {
                            // reached quorum with multiple responses
                            return Poll::Ready(Ok(val));
                        } else {
                            this.responses.push((val, response_weight));
                        }
                    } else if response_weight >= this.inner.quorum_weight {
                        // reached quorum with single response
                        return Poll::Ready(Ok(val));
                    } else {
                        this.responses.push((val, response_weight));
                    }
                }
                Poll::Ready((Err(err), _)) => this.errors.push(err),
                _ => {
                    this.requests.push(request);
                }
            }
        }

        if this.requests.is_empty() {
            // No more requests and no quorum reached
            this.responses.sort_by(|a, b| b.1.cmp(&a.1));
            let responses: Vec<RawResponse> = std::mem::take(&mut this.responses)
                .into_iter()
                .map(|r| r.0)
                .collect();
            let errors = std::mem::take(&mut this.errors);
            Poll::Ready(Err(RpcError::NoQuorumReached { responses, errors }))
        } else {
            Poll::Pending
        }
    }
}

/// The configuration of a provider for the `QuorumProvider`
#[derive(Debug, Clone)]
pub struct WeightedProvider<P> {
    inner: P,
    weight: u64,
}

impl<P> WeightedProvider<P> {
    /// Create a `WeightedProvider` with weight `1`
    pub fn new(inner: P) -> Self {
        Self::with_weight(inner, 1)
    }

    pub fn with_weight(inner: P, weight: u64) -> Self {
        assert!(weight > 0);
        Self { inner, weight }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<P> RpcConnection for QuorumProvider<P>
where
    P: RpcConnection,
{
    async fn _request(&self, request: RawRequest) -> Result<RawResponse, RpcError> {
        let mut request = request.clone();

        self.normalize_request(&mut request).await;

        let requests = self
            .providers
            .iter()
            .enumerate()
            .map(|(idx, provider)| {
                let request = request.clone();
                let fut = provider.inner._request(request).map(move |res| (res, idx));
                Box::pin(fut) as PendingRequest
            })
            .collect::<Vec<_>>();

        Ok(QuorumRequest::new(self, requests).await?)
    }
}

// A stream that returns a value and the weight of its provider
type WeightedNotificationStream =
    Pin<Box<dyn futures_core::Stream<Item = (Notification, u64)> + Send + Unpin + 'static>>;

/// A Subscription stream that only yields the next value if the underlying
/// providers reached quorum.
pub struct QuorumStream {
    // Weight required to reach quorum
    quorum_weight: u64,
    /// The different notifications with their cumulative weight
    responses: HashMap<Notification, u64>,
    /// All provider notification streams
    notifications: Vec<WeightedNotificationStream>,
    /// Outbound channel
    outbound: UnboundedSender<Notification>,
}

impl QuorumStream {
    fn new(
        outbound: UnboundedSender<Notification>,
        quorum_weight: u64,
        notifications: Vec<WeightedNotificationStream>,
    ) -> Self {
        Self {
            quorum_weight,
            responses: HashMap::new(),
            notifications,
            outbound,
        }
    }

    fn spawn(mut self) {
        let fut = async move {
            let mut notifications = select_all(self.notifications).fuse();
            let quorum_weight = self.quorum_weight;
            while let Some((incoming, new_weight)) = notifications.next().await {
                // update our table of responses
                let total_weight = self
                    .responses
                    .entry(incoming.clone())
                    .and_modify(|e| {
                        if *e < quorum_weight {
                            *e += new_weight;
                        }
                    })
                    .or_insert(new_weight);

                // if the total weight was not previously above quorum,
                // but now is, dispatch the notification
                if *total_weight >= quorum_weight && *total_weight - new_weight < quorum_weight {
                    match self.outbound.unbounded_send(incoming) {
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
            }
        };
        #[cfg(target_arch = "wasm32")]
        spawn_local(fut);

        #[cfg(not(target_arch = "wasm32"))]
        tokio::spawn(fut);
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::{EthereumMiddleware, MockRpcConnection};

    use super::{Quorum, QuorumProvider, WeightedProvider};
    use ethers_core::types::U64;

    async fn test_quorum(q: Quorum) {
        let num = 5u64;
        let value = U64::from(42);
        let mut providers = Vec::new();
        let mut mocked = Vec::new();
        for _ in 0..num {
            let mock = MockRpcConnection::new();
            mock.push(value).unwrap();
            providers.push(WeightedProvider::new(mock.clone()));
            mocked.push(mock);
        }
        let quorum = QuorumProvider::builder()
            .add_providers(providers)
            .quorum(q)
            .build();
        let quorum_weight = quorum.quorum_weight;

        let blk = quorum.get_block_number().await.unwrap();
        assert_eq!(blk, value);

        // count the number of providers that returned a value
        let requested = mocked
            .iter()
            .filter(|mock| mock.assert_request("eth_blockNumber", ()).is_ok())
            .count();

        match q {
            Quorum::All => {
                assert_eq!(requested as u64, num);
            }
            Quorum::Majority => {
                assert_eq!(requested as u64, quorum_weight);
            }
            Quorum::Percentage(pct) => {
                let expected = num * (pct as u64) / 100;
                assert_eq!(requested, expected as usize);
            }
            Quorum::ProviderCount(count) => {
                assert_eq!(requested, count);
            }
            Quorum::Weight(w) => {
                assert_eq!(requested as u64, w);
            }
        }
    }

    #[tokio::test]
    async fn majority_quorum() {
        test_quorum(Quorum::Majority).await
    }

    #[tokio::test]
    async fn percentage_quorum() {
        test_quorum(Quorum::Percentage(100)).await
    }

    #[tokio::test]
    async fn count_quorum() {
        test_quorum(Quorum::ProviderCount(3)).await
    }

    #[tokio::test]
    async fn weight_quorum() {
        test_quorum(Quorum::Weight(5)).await
    }

    #[tokio::test]
    async fn all_quorum() {
        test_quorum(Quorum::All).await
    }
}
