## TODOS:

- [x] Implement polling filters
  - [x] modify filter watchers
- [x] PubSub
  - [x] add new traits
  - [x] modify subscription stream
- [ ] Tx Dispatch
  - [x] modify pending tx
  - [ ] fill_transaction
  - [ ] default sender
- [x] Middleware trait
  - [x] modify escalator
  - [x] ENS resolution
- [ ] remaining transports
  - [ ] ipc
  - [ ] mock
  - [ ] quorum
  - [ ] retrying
- [ ] renames
  - [ ] `provider.rs` to `connection.rs`
  - [ ] `transports/` to `connections/`

## Stretch Goals

- [ ] parameterize Middleware with `Network` trait
