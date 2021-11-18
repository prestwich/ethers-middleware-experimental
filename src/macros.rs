macro_rules! impl_rpc_params {
    ($method:literal, $params:ty, $res:ty) => {
        impl crate::types::RequestParams for $params {
            const METHOD: &'static str = $method;
            type Response = $res;
        }
    };
}

macro_rules! decl_rpc_param_type {
    ($method:literal, $name:ident) => {
        paste::paste! {
            #[doc = "RPC Params for `" $method "`"]
            #[derive(Debug, Copy, Clone, serde::Serialize)]
            pub struct [<$name Params>];
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ] ) => {
        paste::paste! {
            #[doc = "RPC Params for `" $method "`"]
            #[derive(Debug, serde::Serialize)]

            pub struct [<$name Params>]  (
                $(pub(crate) $param),*
            );
        }
    };

    ($method:literal, $name:ident, param: $param:ty) => {
        paste::paste! {
            #[doc = "RPC Params for `" $method "`"]
            #[derive(Debug, Clone)]
            pub struct [<$name Params>] ( pub(crate) $param );

            impl From<$param> for [<$name Params>] {
                fn from(p: $param) -> Self {
                    Self(p)
                }
            }

            impl serde::Serialize for [<$name Params>] {
                fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                    [&self.0].serialize(serializer)
                }
            }
        }
    };
}

macro_rules! impl_dispatch_method {
    ($name:ident, $resp:ty) => {
        paste::paste!{
            pub(crate) async fn [<dispatch_ $name:snake>](provider: &dyn crate::provider::RpcConnection) -> Result<Response, crate::error::RpcError> {
                use crate::types::RequestParams;
                [<$name Params>].send_via(provider).await
            }
        }
    };
    ($name:ident, $params:ty, $resp:ty) => {
        paste::paste!{
            pub(crate) async fn [<dispatch_ $name:snake>](provider: &dyn crate::provider::RpcConnection, params: &Params) -> Result<Response,crate::error::RpcError> {
                use crate::types::RequestParams;
                params.send_via(provider).await
            }
        }
    };
}

// // Currently unused, as we never need to in-line declare a response type.
// // All responses have existing types in ethers as far as I can tell
// macro_rules! decl_rpc_response_type {
//     ($method:literal, $name:ident, { $( $resp:ident: $resp_ty:ty, )* }) => {
//         paste::paste! {
//             #[doc = "RPC Response for `" $method "`"]
//             #[derive(Debug, serde::Deserialize)]
//             pub struct [<$name Response>]  {
//                 $($resp: $resp_ty,)*
//             }
//         }
//     };
// }

macro_rules! impl_rpc {
    ($method:literal, $name:ident, response: $resp:ty $(,)?) => {
        paste::paste! {
            #[allow(unused_imports)]
            mod [<inner_ $method:snake>] {
                use super::*;
                decl_rpc_param_type!($method, $name);

                type Params = [<$name Params>];
                type Response = $resp;

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Response);
            }
            pub use [<inner_ $method:snake>]::*;
        }
    };

    ($method:literal, $name:ident, response: { $( $resp:ident: $resp_ty:ty, )* $(,)?}) => {
        paste::paste! {
            mod [<inner_ $method:snake>] {
                use super::*;

                decl_rpc_param_type!($method, $name);
                decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* });

                type Params = [<$name Params>];
                type Response = [<$name Response>];

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Response);
            }
            pub use [<inner_ $method:snake>]::*;
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ], response: $resp:ty $(,)?) => {
        paste::paste! {
            mod [<inner_ $method:snake>] {
                use super::*;
                decl_rpc_param_type!($method, $name, params: [ $($param),* ]);

                type Params = [<$name Params>];
                type Response = $resp;

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Params, Response);
            }
            pub use [<inner_ $method:snake>]::*;
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ], response: { $( $resp:ident: $resp_ty:ty, )* } $(,)?) => {
        paste::paste! {
            mod [<inner_ $method:snake>] {
                use super::*;
                decl_rpc_param_type!($method, $name, params: [ $($param),* ]);
                decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* } );

                type Params = [<$name Params>];
                type Response = [<$name Response>];

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Params, Response);
            }
            pub use [<inner_ $method:snake>]::*;
        }
    };

    ($method:literal, $name:ident, param: $param:ty, response: { $( $resp:ident: $resp_ty:ty, )* } $(,)?) => {
        paste::paste! {
            mod [<inner_ $method:snake>] {
                use super::*;

                decl_rpc_param_type!($method, $name, param: $param);
                decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* });

                type Params = [<$name Params>];
                type Response = [<$name Response>];

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Params, Response);
            }
            pub use [<inner_ $method:snake>]::*;
        }
    };

    ($method:literal, $name:ident, param: $param:ty, response: $resp:ty $(,)?) => {
        paste::paste! {
            mod [<inner_ $method:snake>] {
                use super::*;
                decl_rpc_param_type!($method, $name, param: $param);

                type Params = [<$name Params>];
                type Response = $resp;

                impl_rpc_params!($method, Params, Response);
                impl_dispatch_method!($name, Params, Response);
            }
            pub use [<inner_ $method:snake>]::*;
        }
    };
}
