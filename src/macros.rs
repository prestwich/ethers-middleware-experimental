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
                $($param),*
            );
        }
    };

    ($method:literal, $name:ident, param: $param:ty) => {
        paste::paste! {
            #[doc = "RPC Params for `" $method "`"]
            #[derive(Debug, Clone)]
            pub struct [<$name Params>] ( $param );

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

macro_rules! decl_rpc_response_type {
    ($method:literal, $name:ident, { $( $resp:ident: $resp_ty:ty, )* }) => {
        paste::paste! {
            #[doc = "RPC Response for `" $method "`"]
            #[derive(Debug, serde::Deserialize)]
            pub struct [<$name Response>]  {
                $($resp: $resp_ty,)*
            }
        }
    };
}

macro_rules! impl_rpc {
    ($method:literal, $name:ident, response: $resp:ty $(,)?) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name);
            impl_rpc_params!($method, [<$name Params>], $resp);
        }
    };

    ($method:literal, $name:ident, response: { $( $resp:ident: $resp_ty:ty, )* $(,)?}) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name);
            decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* });
            impl_rpc_params!($method, [<$name Params>], [<$name Response>]);
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ], response: $resp:ty $(,)?) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name, params: [ $($param),* ]);
            impl_rpc_params!($method, [<$name Params>], $resp);
        }
    };

    ($method:literal, $name:ident, params: [ $($param:ty),* ], response: { $( $resp:ident: $resp_ty:ty, )* } $(,)?) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name, params: [ $($param),* ]);
            decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* } );
            impl_rpc_params!($method, [<$name Params>], [<$name Response>]);
        }
    };

    ($method:literal, $name:ident, param: $param:ty, response: { $( $resp:ident: $resp_ty:ty, )* } $(,)?) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name, param: $param);
            decl_rpc_response_type!($method, $name, { $( $resp: $resp_ty, )* });
            impl_rpc_params!($method, [<$name Params>], [<$name Response>]);
        }
    };

    ($method:literal, $name:ident, param: $param:ty, response: $resp:ty $(,)?) => {
        paste::paste! {
            decl_rpc_param_type!($method, $name, param: $param);
            impl_rpc_params!($method, [<$name Params>], $resp);
        }
    };
}
