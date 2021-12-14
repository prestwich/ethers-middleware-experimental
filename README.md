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
- [ ] Ensure tests are enabled
- [ ] Improve rustdoc and doctests
- [ ] backports
  - [x] [dev-rpc](https://github.com/gakonst/ethers-rs/pull/640/)
  - [x] [fix send canceled](https://github.com/gakonst/ethers-rs/commit/8d07610e4a39b461482738dfcb39f88caa60cd67#diff-1fd8e701f7ec17b179805a6da8105e3d441f1d36966dca5021589a06f3c1a9f7)
  - [x] [fix noncetoolow parser](https://github.com/gakonst/ethers-rs/pull/643)
  - [x] [fix noncetoolow2](https://github.com/gakonst/ethers-rs/pull/655/files)
  - [ ] [fee history block count as quantity](https://github.com/gakonst/ethers-rs/pull/668)
  - [ ] [block count as quantity2](https://github.com/gakonst/ethers-rs/pull/669)

## Stretch Goals

- [x] parameterize Middleware with `Network` trait

## Design Notes

- the Middleware traits are re-designed to be object safe
- the root middleware must be an `RpcConnection`
- Other middlewares should nest around that
- for compartmentalization and maintainability, Middleware functionality is
  split up into
  - `BaseMiddleware`
  - `GethMiddleware`
  - `ParityMiddleware`
  - `Middleware`
- Each of these are parameterized with a `Network` ZST that exposes
  network-specific types and functionality
- A top-level `NetworkMiddleware` trait (e.g. `EthereumMiddleware`) abstracts
  these and provides a single import.
  - by default, it delegates to the underlying implementations
  - we recommend middleware implementors override behavior in the
    `BaseMiddleware` or `Middleware` trait, NOT in the `NetworkMiddleware` trait
- the root middleware _may_ be a `PubSubConnection`
  - the `PubSubConnection` trait is primarily concerned with managing response
    channels
  - this allows `PubSubMiddleware` to access notification-based APIs
