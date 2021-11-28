use async_trait::async_trait;
use ethers::prelude::U256;
use futures_channel::{
    mpsc::{self, UnboundedReceiver},
    oneshot,
};
use futures_util::{
    sink::{Sink, SinkExt},
    stream::{Fuse, Stream, StreamExt},
};
use serde_json::Value;
use std::{
    collections::{btree_map::Entry, BTreeMap},
    fmt::{self, Debug},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use crate::{
    error::{RpcError, WsError},
    provider::{PubSubConnection, RpcConnection},
    types::{
        JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, RawRequest, RawResponse,
    },
};

/// Instructions for the `WsServer`.
enum Instruction {
    /// JSON-RPC request
    Request {
        request: JsonRpcRequest,
        sender: ResponseChannel,
    },
    /// Create a new subscription
    Subscribe { id: U256, sink: Subscription },
    /// Cancel an existing subscription
    Unsubscribe { id: U256 },
}

// only used with WS
macro_rules! if_wasm {
    ($($item:item)*) => {$(
        #[cfg(target_arch = "wasm32")]
        $item
    )*}
}

macro_rules! if_not_wasm {
    ($($item:item)*) => {$(
        #[cfg(not(target_arch = "wasm32"))]
        $item
    )*}
}

if_wasm! {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::spawn_local;
    use ws_stream_wasm::*;

    type Message = WsMessage;
    type InternalWsError = ws_stream_wasm::WsErr;
    type WsStreamItem = Message;

    macro_rules! error {
        ( $( $t:tt )* ) => {
            web_sys::console::error_1(&format!( $( $t )* ).into());
        }
    }
    macro_rules! warn {
        ( $( $t:tt )* ) => {
            web_sys::console::warn_1(&format!( $( $t )* ).into());
        }
    }
    macro_rules! debug {
        ( $( $t:tt )* ) => {
            web_sys::console::log_1(&format!( $( $t )* ).into());
        }
    }
}

if_not_wasm! {
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{
            self,
        },
    };
    type Message = tungstenite::protocol::Message;
    type InternalWsError = tungstenite::Error;
    type WsStreamItem = Result<Message, InternalWsError>;
    use tracing::{debug, error, warn};
}

type ResponseChannel = oneshot::Sender<Result<RawResponse, JsonRpcError>>;
type Subscription = mpsc::UnboundedSender<Value>;

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum Incoming {
    Notification(JsonRpcNotification),
    Response(JsonRpcResponse),
}

/// A JSON-RPC Client over Websockets.
///
/// ```no_run
/// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// use ethers_providers::Ws;
///
/// let ws = Ws::connect("wss://localhost:8545").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Ws {
    id: Arc<AtomicU64>,
    instructions: mpsc::UnboundedSender<Instruction>,
}

impl Debug for Ws {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebsocketProvider")
            .field("id", &self.id)
            .finish()
    }
}

impl Ws {
    /// Initializes a new WebSocket Client, given a Stream/Sink Websocket implementer.
    /// The websocket connection must be initiated separately.
    pub fn new<S: 'static>(ws: S) -> Self
    where
        S: Send
            + Sync
            + Stream<Item = WsStreamItem>
            + Sink<Message, Error = InternalWsError>
            + Unpin,
    {
        let (sink, stream) = mpsc::unbounded();

        // Spawn the server
        WsServer::new(ws, stream).spawn();

        Self {
            id: Arc::new(AtomicU64::new(0)),
            instructions: sink,
        }
    }

    /// Returns true if the WS connection is active, false otherwise
    pub fn ready(&self) -> bool {
        !self.instructions.is_closed()
    }

    /// Initializes a new WebSocket Client
    #[cfg(target_arch = "wasm32")]
    pub async fn connect(url: &str) -> Result<Self, WsError> {
        let (_, wsio) = WsMeta::connect(url, None)
            .await
            .expect_throw("Could not create websocket");

        Ok(Self::new(wsio))
    }

    /// Initializes a new WebSocket Client
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn connect(
        url: impl tungstenite::client::IntoClientRequest + Unpin,
    ) -> Result<Self, WsError> {
        let (ws, _) = connect_async(url).await?;
        Ok(Self::new(ws))
    }

    fn send(&self, msg: Instruction) -> Result<(), WsError> {
        self.instructions
            .unbounded_send(msg)
            .map_err(|err| WsError::ChannelError(format!("{:?}", err)))
    }
}

#[async_trait]
impl RpcConnection for Ws {
    async fn _request(&self, request: RawRequest) -> Result<RawResponse, RpcError> {
        let id = self.id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = oneshot::channel();

        let request = JsonRpcRequest {
            id,
            jsonrpc: "2.0",
            request,
        };

        self.send(Instruction::Request { request, sender })?;

        Ok(receiver.await??)
    }
}

impl PubSubConnection for Ws {
    fn close_listener(&self, id: U256) -> Result<(), RpcError> {
        Ok(self.send(Instruction::Unsubscribe { id: id.into() })?)
    }

    fn install_listener(&self, id: U256) -> Result<UnboundedReceiver<Value>, RpcError> {
        let (sink, stream) = mpsc::unbounded();

        self.send(Instruction::Subscribe { id, sink })
            .map_err(|e| WsError::ChannelError(format!("{:?}", e)))?;
        Ok(stream)
    }
}

struct WsServer<S> {
    ws: Fuse<S>,
    instructions: Fuse<mpsc::UnboundedReceiver<Instruction>>,

    pending: BTreeMap<u64, ResponseChannel>,
    subscriptions: BTreeMap<U256, Subscription>,
}

impl<S> WsServer<S>
where
    S: Send + Sync + Stream<Item = WsStreamItem> + Sink<Message, Error = InternalWsError> + Unpin,
{
    /// Instantiates the Websocket Server
    fn new(ws: S, requests: mpsc::UnboundedReceiver<Instruction>) -> Self {
        Self {
            // Fuse the 2 steams together, so that we can `select` them in the
            // Stream implementation
            ws: ws.fuse(),
            instructions: requests.fuse(),
            pending: BTreeMap::default(),
            subscriptions: BTreeMap::default(),
        }
    }

    /// Returns whether the all work has been completed.
    ///
    /// If this method returns `true`, then the `instructions` channel has been closed and all
    /// pending requests and subscriptions have been completed.
    fn is_done(&self) -> bool {
        self.instructions.is_done() && self.pending.is_empty() && self.subscriptions.is_empty()
    }

    /// Spawns the event loop
    fn spawn(mut self)
    where
        S: 'static,
    {
        let f = async move {
            loop {
                if self.is_done() {
                    debug!("work complete");
                    break;
                }
                match self.tick().await {
                    Err(WsError::UnexpectedClose) => {
                        error!("{}", WsError::UnexpectedClose);
                        break;
                    }
                    Err(e) => {
                        panic!("WS Server panic: {}", e);
                    }
                    _ => {}
                }
            }
        };

        #[cfg(target_arch = "wasm32")]
        spawn_local(f);

        #[cfg(not(target_arch = "wasm32"))]
        tokio::spawn(f);
    }

    // dispatch an RPC request
    async fn service_request(
        &mut self,
        request: JsonRpcRequest,
        sender: ResponseChannel,
    ) -> Result<(), WsError> {
        if self.pending.insert(request.id, sender).is_some() {
            warn!("Replacing a pending request with id {:?}", request.id);
        }

        let text = serde_json::to_string(&request).expect("ser does not fail");

        if let Err(e) = self.ws.send(Message::Text(text)).await {
            error!("WS connection error: {:?}", e);
            self.pending.remove(&request.id);
        }
        Ok(())
    }

    /// Dispatch a subscription request
    async fn service_subscribe(&mut self, id: U256, sink: Subscription) -> Result<(), WsError> {
        if self.subscriptions.insert(id, sink).is_some() {
            warn!("Replacing already-registered subscription with id {:?}", id);
        }
        Ok(())
    }

    /// Dispatch a unsubscribe request
    async fn service_unsubscribe(&mut self, id: U256) -> Result<(), WsError> {
        if self.subscriptions.remove(&id).is_none() {
            warn!(
                "Unsubscribing from non-existent subscription with id {:?}",
                id
            );
        }
        Ok(())
    }

    /// Dispatch an outgoing message
    async fn service(&mut self, instruction: Instruction) -> Result<(), WsError> {
        match instruction {
            Instruction::Request { request, sender } => self.service_request(request, sender).await,
            Instruction::Subscribe { id, sink } => self.service_subscribe(id, sink).await,
            Instruction::Unsubscribe { id } => self.service_unsubscribe(id).await,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn handle_ping(&mut self, inner: Vec<u8>) -> Result<(), WsError> {
        self.ws.send(Message::Pong(inner)).await?;
        Ok(())
    }

    async fn handle_text(&mut self, inner: String) -> Result<(), WsError> {
        match serde_json::from_str::<Incoming>(&inner) {
            Err(_) => {
                // Ignore deser errors
            }
            Ok(Incoming::Response(resp)) => {
                if let Some(request) = self.pending.remove(&resp.id) {
                    request
                        .send(Ok(resp.result))
                        .map_err(|e| WsError::ChannelError(format!("{:?}", e)))?;
                }
            }
            Ok(Incoming::Notification(notification)) => {
                let id = notification.params.subscription;
                if let Entry::Occupied(stream) = self.subscriptions.entry(id) {
                    if let Err(err) = stream.get().unbounded_send(notification.params.result) {
                        if err.is_disconnected() {
                            // subscription channel was closed on the receiver end
                            stream.remove();
                        }
                        return Err(WsError::ChannelError(format!("{:?}", err)));
                    }
                }
            }
        }
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    async fn handle(&mut self, resp: Message) -> Result<(), WsError> {
        match resp {
            Message::Text(inner) => self.handle_text(inner).await,
            Message::Binary(buf) => Err(WsError::UnexpectedBinary(buf)),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn handle(&mut self, resp: Message) -> Result<(), WsError> {
        match resp {
            Message::Text(inner) => self.handle_text(inner).await,
            Message::Ping(inner) => self.handle_ping(inner).await,
            Message::Pong(_) => Ok(()), // Server is allowed to send unsolicited pongs.
            Message::Close(Some(frame)) => Err(WsError::WsClosed(frame)),
            Message::Close(None) => Err(WsError::UnexpectedClose),
            Message::Binary(buf) => Err(WsError::UnexpectedBinary(buf)),
        }
    }

    /// Processes 1 instruction or 1 incoming websocket message
    #[allow(clippy::single_match)]
    #[cfg(target_arch = "wasm32")]
    async fn tick(&mut self) -> Result<(), WsError> {
        futures_util::select! {
            // Handle requests
            instruction = self.instructions.select_next_some() => {
                self.service(instruction).await?;
            },
            // Handle ws messages
            resp = self.ws.next() => match resp {
                Some(resp) => self.handle(resp).await?,
                None => {
                    return Err(WsError::UnexpectedClose);
                },
            }
        };

        Ok(())
    }

    /// Processes 1 instruction or 1 incoming websocket message
    #[allow(clippy::single_match)]
    #[cfg(not(target_arch = "wasm32"))]
    async fn tick(&mut self) -> Result<(), WsError> {
        futures_util::select! {
            // Handle requests
            instruction = self.instructions.select_next_some() => {
                self.service(instruction).await?;
            },
            // Handle ws messages
            resp = self.ws.next() => match resp {
                Some(Ok(resp)) => self.handle(resp).await?,
                // TODO: Log the error?
                Some(Err(_)) => {},
                None => {
                    return Err(WsError::UnexpectedClose);
                },
            }
        };

        Ok(())
    }
}

#[cfg(test)]
#[cfg(not(feature = "celo"))]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use crate::middleware::BaseMiddleware;
    use ethers::core::{
        types::{Block, TxHash, U256},
        utils::Ganache,
    };

    #[tokio::test]
    async fn request() {
        let ganache = Ganache::new().block_time(1u64).spawn();
        let ws = Ws::connect(ganache.ws_endpoint()).await.unwrap();

        let block_num = ws.get_block_number().await.unwrap();
        std::thread::sleep(std::time::Duration::new(3, 0));
        let block_num2 = ws.get_block_number().await.unwrap();
        assert!(block_num2 > block_num);
    }

    #[tokio::test]
    async fn subscription() {
        // let ganache = Ganache::new().block_time(1u64).spawn();
        // let ws = Ws::connect(ganache.ws_endpoint()).await.unwrap();

        todo!()
        // // Subscribing requires sending the sub request and then subscribing to
        // // the returned sub_id
        // let sub_id: U256 = ws.request("eth_subscribe", ["newHeads"]).await.unwrap();
        // let mut stream = ws.subscribe(sub_id).unwrap();

        // let mut blocks = Vec::new();
        // for _ in 0..3 {
        //     let item = stream.next().await.unwrap();
        //     let block = serde_json::from_value::<Block<TxHash>>(item).unwrap();
        //     blocks.push(block.number.unwrap_or_default().as_u64());
        // }

        // assert_eq!(sub_id, 1.into());
        // assert_eq!(blocks, vec![1, 2, 3])
    }
}
