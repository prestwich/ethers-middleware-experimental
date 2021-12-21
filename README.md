# Clients for interacting with Ethereum-like nodes

This crate provides asynchronous
[Ethereum JSON-RPC](https://github.com/ethereum/wiki/wiki/JSON-RPC) compliant
clients.

For more documentation on the available calls, refer to the
[`Middleware`](./middleware/trait.Middleware.html) trait.

## Standard usage

Most users will simply import `Middleware`and their RPC connection of choice.

```no_run
use ethers_providers::{Http, Middleware};

# async fn foo() -> Result<(), Box<dyn std::error::Error>> {
let provider: Http =
  "https://mainnet.infura.io/v3/c60b0bb42f8a4c6481ecd229eddaca27".parse()?;

let block = provider.get_block(100u64.into()).await?;
println!("Got block: {}", serde_json::to_string(&block)?);

let code = provider.get_code("0x89d24a6b4ccb1b6faa2625fe562bdd9a23260359".parse()?, None).await?;
println!("Got code: {}", serde_json::to_string(&code)?);
# Ok(())
# }
```

## Websockets

The crate has support for WebSockets via Tokio.

```no_run
# async fn foo() -> Result<(), Box<dyn std::error::Error>> {
use ethers_providers::Ws;
let ws = Ws::connect("ws://localhost:8545").await?;
# Ok(())
# }
```

## Ethereum Name Service

The provider may also be used to resolve
[Ethereum Name Service](https://ens.domains) (ENS) names to addresses (and vice
versa). The default ENS address is
[mainnet](https://etherscan.io/address/0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e)
and can be overriden by passing a registry address to the
[`ens_resolve`](./middleware/trait.Middleware.html#method.ens_resolve) or
[`ens_lookup`](./middleware/trait.Middleware.html#method.ens_lookup)
methods on the middleware.

```no_run
# use ethers_providers::{Http, Middleware};
# use std::convert::TryFrom;
# async fn foo() -> Result<(), Box<dyn std::error::Error>> {
# let provider: Http =
#  "https://mainnet.infura.io/v3/c60b0bb42f8a4c6481ecd229eddaca27".parse()?;

// Resolve ENS name to Address
let name = "vitalik.eth";
let address = provider.ens_resolve(None, name).await?;

// Lookup ENS name given Address
let resolved_name = provider.ens_lookup(None, address).await?;
assert_eq!(name, resolved_name);

# Ok(())
# }
```

# Design Notes

This crate is designed to provide a great degree of flexibility. It aims to
allow for many EVM-like networks with slightly different RPC types. As a result
many of its core types are generic over a
[`Network`](./networks/trait.Network.html) trait. This trait contains all
network-specific type information.

While flexibility is important, we understand that most users of this library
are Ethereum-oriented. As a result, **we do not export generic structs or
traits at the crate root** level. Instead, all generic types are in modules.
**The crate root exports ONLY Ethereum-parameterized structs and traits.**

## Connections

A "connection" is the struct that actually dispatches RPC requests to and from
the node. The core of a middleware stack must be an
[`RpcConnection`](connections/trait.RpcConnection.html). Some connections are
[`PubSubConnections`](connections/trait.PubSubConnection.html), and support
notification-based subscriptions.

Common connections are [`Http`](connections/struct.Http.html) &
[`Websockets`](connections/struct.Ws.html). We've also provided some useful
connection abstractions like
[a simple retrying connection](connections/struct.RetryingProvider.html) and
[agreement across a quorum](connections/struct.QuorumProvider.html).

> > 🚨 **Tip**:
> >
> > There is a blanket implementation of the
> > [`Middleware`](middleware/trait.Middleware.html) traits on all
> > connections. Implementing
> > [`RpcConnection`](connections/trait.RpcConnection.html) gets you all
> > middleware traits for free.

## Middleware Trait design

For compartmentalization and maintainability, Middleware functionality is split
up into the following traits:

- [`BaseMiddleware`](middleware/trait.BaseMiddleware.html) - Core RPC features
- [`GethMiddleware`](middleware/trait.GethMiddleware.html) - Geth-specific
  features (e.g. txpool RPC)
- [`ParityMiddleware`](middleware/trait.ParityMiddleware.html) -
  Parity-specific features
- [`Middleware`](middleware/trait.Middleware.html) - High-level abstractions
  for RPC features (e.g ens resolution and
  [`PendingTransaction`](watchers/pending_transaction/struct.GenericPendingTransaction.html))
- Network-parameterized middleware - Macro-generated traits which contains all
  methods on the above, parameterized by a specific network.

Each of these traits is designed to be object-safe, and allow easy upcasting to less-specific traits.

> > 🚨 **Tip**:
> >
> > Most users will use ONLY the
> > [`EthereumMiddleware`](networks/ethereum/trait.EthereumMiddleware.html) and
> > [`EthereumPubSubMiddleware`](networks/ethereum/trait.EthereumPubSubMiddleware.html)
> > traits, which are re-exported under the `ethers_providers::Middleware` and
> > `ethers_providers::PubSubMiddleware` names. Only middleware implementors
> > need to worry about generics :)

## Implementing Middleware

We recommend middleware implementors override behavior in the **generic**
`Middleware` traits, **NOT** the network-paramterized `Middleware` trait. In
other words, we _do not recommend_ `impl crate::Middleware for T`. We
_strongly recommend_ implementing `impl crate::middleware::Middleware<N> for T`.

This ensures that delegation works properly and that your middleware can be used across networks.

> > 🚨 **Tip**:
> >
> > There's a blanket implementation of
> > [`EthereumMiddleware`](networks/ethereum/trait.EthereumMiddleware.html)
> > on all types that implement the generic middleware traits. Just implement
> > the generic middlewares and you get the network middlewares for free :)

```no_run
// The generic versions
use ethers_providers::{
    Network,
    middleware::{
      BaseMiddleware,
      GethMiddleware,
      Middleware,
      ParityMiddleware,
    },
};

# #[derive(Debug, Copy, Clone)]
pub struct MyMiddleware;

// We recommend implementing each of these.
impl<N> BaseMiddleware<N> for MyMiddleware
where
    N: Network
{
    /* ... */
#     fn inner_base(&self) -> &dyn BaseMiddleware<N> {
#        self
#    }
#    fn provider(&self) -> &dyn ethers_providers::RpcConnection {
#        unimplemented!()
#    }
}

impl<N> GethMiddleware<N> for MyMiddleware
where
    N: Network
{
    /* ... */
#    fn inner_geth(&self) -> &dyn GethMiddleware<N> {
#        self
#    }

#    fn as_base_middleware(&self) -> &dyn BaseMiddleware<N> {
#        self
#    }
}

impl<N> ParityMiddleware<N> for MyMiddleware
where
    N: Network
{
    /* ... */
#    fn inner_parity(&self) -> &dyn ParityMiddleware<N> {
#        self
#    }

#    fn as_base_middleware(&self) -> &dyn BaseMiddleware<N> {
#        self
#    }
}


impl<N> Middleware<N> for MyMiddleware
where
    N: Network
{
    /* ... */
#    fn inner(&self) -> &dyn Middleware<N> {
#        self
#    }
#    fn as_base_middleware(&self) -> &dyn BaseMiddleware<N> {
#        self
#    }
#    fn as_geth_middleware(&self) -> &dyn GethMiddleware<N> {
#        self
#    }
#    fn as_parity_middleware(&self) -> &dyn ParityMiddleware<N> {
#        self
#    }
}
```

## TODOS:

- [x] Implement polling filters
  - [x] modify filter watchers
- [x] PubSub
  - [x] add new traits
  - [x] modify subscription stream
- [x] Tx Dispatch
  - [x] modify pending tx
  - [x] fill_transaction
  - [x] default sender
- [x] Middleware trait
  - [x] modify escalator
  - [x] ENS resolution
- [x] remaining transports
  - [x] ipc
  - [x] mock
  - [x] quorum
  - [x] retrying
- [x] renames
  - [x] `provider.rs` to `connections/mod.rs`
  - [x] `transports/` to `connections/`
- [x] Ensure tests are enabled
- [x] Improve rustdoc and doctests
- [ ] backports
  - [x] [dev-rpc](https://github.com/gakonst/ethers-rs/pull/640/)
  - [x] [fix send canceled](https://github.com/gakonst/ethers-rs/commit/8d07610e4a39b461482738dfcb39f88caa60cd67#diff-1fd8e701f7ec17b179805a6da8105e3d441f1d36966dca5021589a06f3c1a9f7)
  - [x] [fix noncetoolow parser](https://github.com/gakonst/ethers-rs/pull/643)
  - [x] [fix noncetoolow2](https://github.com/gakonst/ethers-rs/pull/655/files)
  - [x] [fee history block count as quantity](https://github.com/gakonst/ethers-rs/pull/668)
  - [x] [block count as quantity2](https://github.com/gakonst/ethers-rs/pull/669)

## Stretch Goals

- [x] parameterize Middleware with `Network` trait
