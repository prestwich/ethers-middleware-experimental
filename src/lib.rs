//! Toweth

use std::{
    error::Error,
    fmt::Debug,
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
    task::Poll,
};

use ethers::prelude::{Transaction, H256, U256};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use tower::Service;
use url::Url;

type PinBoxFut<A, B> = Pin<Box<dyn Future<Output = Result<A, B>>>>;

#[derive(Debug)]
pub struct JsonRpcClient {
    id: AtomicU64,
    client: reqwest::Client,
    url: Url,
}

impl JsonRpcClient {
    pub fn new(url: Url) -> Self {
        let id = 0u64.into();
        let client = Default::default();
        Self { id, client, url }
    }

    fn next_id(&mut self) -> u64 {
        self.id.fetch_add(1, Relaxed)
    }
}

impl Service<Value> for JsonRpcClient {
    type Error = Box<dyn std::error::Error>;

    type Response = Value;

    type Future = PinBoxFut<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Value) -> Self::Future {
        let fut = self
            .client
            .post(self.url.as_ref())
            .json(&req.to_string())
            .send();

        Box::pin(async {
            let res = fut
                .await
                .map_err(Box::new)?
                .text()
                .await
                .map_err(Box::new)?;

            serde_json::from_str(&res).map_err(|e| Box::new(e) as Box<dyn Error>)
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct RawJsonRpcRequest<'a> {
    id: u64,
    jsonrpc: &'static str,
    method: &'a str,
    params: Value,
}

impl<'a> RawJsonRpcRequest<'a> {
    fn new(id: u64, method: &'a str, params: Value) -> Self {
        Self {
            id,
            jsonrpc: "2.0",
            method,
            params,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RawJsonRpcResponse {
    pub(crate) id: u64,
    jsonrpc: String,
    #[serde(flatten)]
    pub data: Value,
}

impl Service<RawJsonRpcRequest<'_>> for JsonRpcClient {
    type Response = RawJsonRpcResponse;

    type Error = Box<dyn Error>;

    type Future = PinBoxFut<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: RawJsonRpcRequest<'_>) -> Self::Future {
        let fut = self.call(serde_json::to_value(req).expect("!ser"));
        Box::pin(async move {
            let res = fut.await?;
            Ok(serde_json::from_value(res)?)
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ResponseData<R> {
    Success { result: R },
    Error { error: JsonRpcError },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

pub trait RpcParam: Debug + Serialize + Send + Sync {
    const METHOD: &'static str;
    type Response: DeserializeOwned;
}

impl<T> Service<T> for JsonRpcClient
where
    T: RpcParam,
{
    type Response = ResponseData<T::Response>;

    type Error = Box<dyn std::error::Error>;

    type Future = PinBoxFut<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: T) -> Self::Future {
        let id = self.next_id();

        let params = serde_json::to_value(req).expect("Types don't fail serializing");
        let req = RawJsonRpcRequest::new(id, T::METHOD, params);

        let fut = self.call(req);
        Box::pin(async move {
            let res = fut.await?;
            Ok(serde_json::from_value(res.data).map_err(Box::new)?)
        })
    }
}

macro_rules! impl_rpc_params {
    ($method:literal, $params:ty, $res:ty) => {
        impl RpcParam for $params {
            const METHOD: &'static str = $method;
            type Response = $res;
        }
    };
}

macro_rules! decl_rpc_param_type {
    ($method:literal, $name:ident) => {
        paste::paste! {
            #[doc = "RPC Params for `" $method]
            #[derive(Debug, Copy, Clone, Serialize, Deserialize)]
            #[serde(into = "Vec<()>", from = "Vec<()>")]
            pub struct [<$name Params>] ();

            impl Into<Vec<()>> for [<$name Params>] {
                fn into(self) -> Vec<()> {
                    Vec::new()
                }
            }

            impl From<Vec<()>> for [<$name Params>] {
                fn from(_: Vec<()>) -> Self {
                    Self()
                }
            }
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ] ) => {
        paste::paste! {
            #[doc = "RPC Params for `" $method]
            #[derive(Debug, Serialize, Deserialize)]
            pub struct [<$name Params>]  (
                $($param),*
            );
        }
    };

    ($method:literal, $name:ident, param: $param:ty) => {
        paste::paste! {
            #[doc = "RPC Params for `" $method]
            #[derive(Debug, Clone)]
            pub struct [<$name Params>] ( $param );

            impl From<$param> for [<$name Params>] {
                fn from(p: $param) -> Self {
                    Self(p)
                }
            }

            impl Serialize for [<$name Params>]
            {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: serde::ser::Serializer,
                {
                    use serde::ser::SerializeSeq;
                    let mut seq = serializer.serialize_seq(Some(1))?;
                    seq.serialize_element(&self.0)?;
                    seq.end()
                }
            }

            impl<'de> Deserialize<'de> for [<$name Params>]
            {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: serde::de::Deserializer<'de>,
                {
                    let mut v = Vec::<$param>::deserialize(deserializer)?;
                    let item = v.drain(..1).next(); // first item if any
                    match item {
                        Some(item) => Ok(Self(item)),
                        None => Err(serde::de::Error::custom("empty"))
                    }
                }
            }
        }
    };
}

macro_rules! decl_rpc_response_type {
    ($method:literal, $name:ident, { $( $resp:ident: $resp_ty:ty, )* }) => {
        paste::paste! {
            #[doc = "RPC Response for `" $method]
            #[derive(Debug, Serialize, Deserialize)]
            pub struct [<$name Response>]  {
                $($resp: $resp_ty,)*
            }
        }
    };
}

macro_rules! impl_rpc {
    ($method:literal, $name:ident, response: $resp:ty) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name);
            impl_rpc_params!($method, [<$name Params>], $resp);
        }
    };

    ($method:literal, $name:ident, response: { $( $resp:ident: $resp_ty:ty, )* }) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name);
            decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* });
            impl_rpc_params!($method, [<$name Params>], [<$name Response>]);
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ], response: $resp:ty) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name, params: [ $($param),* ]);
            impl_rpc_params!($method, [<$name Params>], $resp);
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ], response: { $( $resp:ident: $resp_ty:ty, )* }) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name, params: [ $($param),* ]);
            decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* } );
            impl_rpc_params!($method, [<$name Params>], [<$name Response>]);
        }
    };

    ($method:literal, $name:ident, param: $param:ty, response: { $( $resp:ident: $resp_ty:ty, )* }) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name, param: $param);
            decl_rpc_response_type!($method, $name, { $( $resp:ident: $resp_ty:ty, )* });
            impl_rpc_params!($method, $params, [<$name Response>]);
        }
    };

    ($method:literal, $name:ident, param: $param:ty, response: $resp:ty) => {
        decl_rpc_param_type!($method, $name, param: $param);
        impl_rpc_params!($method, $param, $resp);
    };
}

impl_rpc!("eth_blockNumber", BlockNumber, response: U256);

impl_rpc!(
    "eth_getTransactionByHash",
    TransactionByHash,
    param: H256,
    response: Option<Transaction>
);

impl_rpc!(
    "eth_doStuff",
    DoStuff,
    params: [ H256, H256, H256 ],
    response: { hello: H256, }
);

#[allow(dead_code)]
async fn compile_check(mut s: JsonRpcClient) {
    let _r = s.call(BlockNumberParams()).await;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_prints() {
        println!(
            "{}",
            serde_json::to_string_pretty(&BlockNumberParams()).unwrap()
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&TransactionByHashParams(H256::zero())).unwrap()
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&DoStuffParams(
                H256::zero(),
                H256::zero(),
                H256::zero(),
            ))
            .unwrap()
        );

        println!(
            "{}",
            serde_json::to_string_pretty(&TransactionByHashParams(H256::zero())).unwrap()
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&DoStuffResponse{hello: H256::zero()})
            .unwrap()
        );
    }
}
