use crate::{
    connections::{PubSubConnection, RpcConnection},
    error::{IpcError, RpcError},
    types::{
        Instruction, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, Notification,
        RawRequest, RawResponse, ResponseChannel, Subscription,
    },
};

use ethers_core::types::U256;

use futures_channel::{
    mpsc::{self, UnboundedReceiver},
    oneshot,
};
use futures_util::stream::{Fuse, StreamExt};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf},
    net::UnixStream,
};
use tokio_util::io::ReaderStream;
use tracing::{error, warn};

/// Unix Domain Sockets (IPC) transport.
#[derive(Debug, Clone)]
pub struct Ipc {
    id: Arc<AtomicU64>,
    messages_tx: mpsc::UnboundedSender<Instruction>,
}

impl Ipc {
    /// Creates a new IPC transport from a Async Reader / Writer
    fn new<S: AsyncRead + AsyncWrite + Send + 'static>(stream: S) -> Self {
        let id = Arc::new(AtomicU64::new(1));
        let (messages_tx, messages_rx) = mpsc::unbounded();

        IpcServer::new(stream, messages_rx).spawn();
        Self { id, messages_tx }
    }

    /// Creates a new IPC transport from a given path using Unix sockets
    #[cfg(unix)]
    pub async fn connect<P: AsRef<Path>>(path: P) -> Result<Self, IpcError> {
        let ipc = UnixStream::connect(path).await?;
        Ok(Self::new(ipc))
    }

    fn send(&self, msg: Instruction) -> Result<(), IpcError> {
        self.messages_tx
            .unbounded_send(msg)
            .map_err(|_| IpcError::ChannelError("IPC server receiver dropped".to_string()))?;

        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RpcConnection for Ipc {
    async fn _request(&self, request: RawRequest) -> Result<RawResponse, RpcError> {
        let id = self.id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = oneshot::channel();

        let request = JsonRpcRequest { id, jsonrpc: "2.0", request };

        self.send(Instruction::Request { request, sender })?;

        Ok(receiver.await??)
    }
}

impl PubSubConnection for Ipc {
    fn uninstall_listener(&self, id: U256) -> Result<(), RpcError> {
        Ok(self.send(Instruction::Unsubscribe { id })?)
    }

    fn install_listener(&self, id: U256) -> Result<UnboundedReceiver<Notification>, RpcError> {
        let (sink, stream) = mpsc::unbounded();

        self.send(Instruction::Subscribe { id, sink })?;
        Ok(stream)
    }
}

struct IpcServer<T> {
    socket_reader: Fuse<ReaderStream<ReadHalf<T>>>,
    socket_writer: WriteHalf<T>,
    requests: Fuse<mpsc::UnboundedReceiver<Instruction>>,
    pending: BTreeMap<u64, ResponseChannel>,
    subscriptions: BTreeMap<U256, Subscription>,
}

impl<T> IpcServer<T>
where
    T: AsyncRead + AsyncWrite,
{
    /// Instantiates the Websocket Server
    pub fn new(ipc: T, requests: mpsc::UnboundedReceiver<Instruction>) -> Self {
        let (socket_reader, socket_writer) = tokio::io::split(ipc);
        let socket_reader = ReaderStream::new(socket_reader).fuse();
        Self {
            socket_reader,
            socket_writer,
            requests: requests.fuse(),
            pending: BTreeMap::default(),
            subscriptions: BTreeMap::default(),
        }
    }

    /// Spawns the event loop
    fn spawn(mut self)
    where
        T: 'static + Send,
    {
        let f = async move {
            let mut read_buffer = Vec::new();
            loop {
                let closed = self.tick(&mut read_buffer).await.expect("WS Server panic");
                if closed && self.pending.is_empty() {
                    break
                }
            }
        };

        tokio::spawn(f);
    }

    /// Processes 1 item selected from the incoming `requests` or `socket`
    #[allow(clippy::single_match)]
    async fn tick(&mut self, read_buffer: &mut Vec<u8>) -> Result<bool, IpcError> {
        futures_util::select! {
            // Service outgoing requests
            msg = self.requests.next() => match msg {
                Some(msg) => self.service_request(msg).await?,
                None => return Ok(true),
            },
            // Handle incoming socket messages
            msg = self.socket_reader.next() => match msg {
                Some(Ok(msg)) => self.handle(read_buffer, msg)?,
                Some(Err(err)) => {
                    error!("IPC read error: {:?}", err);
                    return Err(err.into());
                },
                None => {},
            },
            // finished
            complete => {},
        };

        Ok(false)
    }

    async fn service_request(&mut self, msg: Instruction) -> Result<(), IpcError> {
        match msg {
            Instruction::Request { request, sender } => {
                if self.pending.insert(request.id, sender).is_some() {
                    warn!("Replacing a ResponseChannel request with id {:?}", request.id);
                }

                if let Err(err) =
                    self.socket_writer.write(serde_json::to_string(&request)?.as_bytes()).await
                {
                    error!("IPC connection error: {:?}", err);
                    self.pending.remove(&request.id);
                }
            }
            Instruction::Subscribe { id, sink } => {
                if self.subscriptions.insert(id, sink).is_some() {
                    warn!("Replacing already-registered subscription with id {:?}", id);
                }
            }
            Instruction::Unsubscribe { id } => {
                if self.subscriptions.remove(&id).is_none() {
                    warn!("Unsubscribing from non-existent subscription with id {:?}", id);
                }
            }
        };

        Ok(())
    }

    fn handle(&mut self, read_buffer: &mut Vec<u8>, bytes: bytes::Bytes) -> Result<(), IpcError> {
        // Extend buffer of previously unread with the new read bytes
        read_buffer.extend_from_slice(&bytes);

        let read_len = {
            // Deserialize as many full elements from the stream as exists
            let mut de: serde_json::StreamDeserializer<_, serde_json::Value> =
                serde_json::Deserializer::from_slice(read_buffer).into_iter();

            // Iterate through these elements, and handle responses/notifications
            while let Some(Ok(value)) = de.next() {
                if let Ok(notification) =
                    serde_json::from_value::<JsonRpcNotification>(value.clone())
                {
                    // Send notify response if okay.
                    if let Err(e) = self.handle_notification(notification) {
                        error!("Failed to send IPC notification: {}", e)
                    }
                } else if let Ok(response) = serde_json::from_value::<JsonRpcResponse>(value) {
                    if let Err(e) = self.handle_response(response) {
                        error!("Failed to send IPC response: {}", e)
                    }
                } else {
                    warn!("JSON from IPC stream is not a response or notification");
                }
            }

            // Get the offset of bytes to handle partial buffer reads
            de.byte_offset()
        };

        // Reset buffer to just include the partial value bytes.
        read_buffer.copy_within(read_len.., 0);
        read_buffer.truncate(read_buffer.len() - read_len);

        Ok(())
    }

    /// Sends notification through the channel based on the ID of the subscription.
    /// This handles streaming responses.
    fn handle_notification(&mut self, notification: JsonRpcNotification) -> Result<(), IpcError> {
        let id = notification.params.subscription;
        if let Some(tx) = self.subscriptions.get(&id) {
            tx.unbounded_send(notification.params).map_err(|_| {
                IpcError::ChannelError(format!("Subscription receiver {} dropped", id))
            })?;
        }

        Ok(())
    }

    /// Sends JSON response through the channel based on the ID in that response.
    /// This handles RPC calls with only one response, and the channel entry is dropped after
    /// sending.
    fn handle_response(&mut self, response: JsonRpcResponse) -> Result<(), IpcError> {
        let id = response.id;

        let response_tx = self.pending.remove(&id).ok_or_else(|| {
            IpcError::ChannelError("No response channel exists for the response ID".to_string())
        })?;

        response_tx.send(Ok(response.result)).map_err(|_| {
            IpcError::ChannelError("Receiver channel for response has been dropped".to_string())
        })?;

        Ok(())
    }
}

#[cfg(all(test, unix))]
#[cfg(not(feature = "celo"))]
mod test {
    use super::*;
    use ethers_core::{
        types::{Block, TxHash, U256, U64},
        utils::Geth,
    };
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn request() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.into_temp_path().to_path_buf();
        let _geth = Geth::new().block_time(1u64).ipc_path(&path).spawn();
        let ipc = Ipc::connect(path).await.unwrap();

        let block_num: U256 =
            ipc.call_method("eth_blockNumber", ()).await.unwrap().deserialize().unwrap();

        std::thread::sleep(std::time::Duration::new(3, 0));
        let block_num2: U256 =
            ipc.call_method("eth_blockNumber", ()).await.unwrap().deserialize().unwrap();
        assert!(block_num2 > block_num);
    }

    #[tokio::test]
    async fn subscription() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.into_temp_path().to_path_buf();
        let _geth = Geth::new().block_time(2u64).ipc_path(&path).spawn();
        let ipc = Ipc::connect(path).await.unwrap();

        let sub_id: U256 =
            ipc.call_method("eth_subscribe", ["newHeads"]).await.unwrap().deserialize().unwrap();
        let mut stream = ipc.install_listener(sub_id).unwrap();

        // Subscribing requires sending the sub call_method and then subscribing to
        // the returned sub_id
        let block_num: u64 = ipc
            .call_method("eth_blockNumber", ())
            .await
            .unwrap()
            .deserialize::<U64>()
            .unwrap()
            .as_u64();
        let mut blocks = Vec::new();
        for _ in 0..3 {
            let item = stream.next().await.unwrap();
            let block = serde_json::from_value::<Block<TxHash>>(item.result).unwrap();
            blocks.push(block.number.unwrap_or_default().as_u64());
        }
        let offset = blocks[0] - block_num;
        assert_eq!(blocks, &[block_num + offset, block_num + offset + 1, block_num + offset + 2])
    }
}
