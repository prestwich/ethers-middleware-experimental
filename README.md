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
- [ ] remaining transports
  - [x] ipc
  - [ ] mock
  - [ ] quorum
  - [ ] retrying
- [x] renames
  - [x] `provider.rs` to `connections/mod.rs`
  - [x] `transports/` to `connections/`

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
