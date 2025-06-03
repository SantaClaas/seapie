use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod mcp {
    use std::{collections::HashMap, default, marker::PhantomData, sync::Arc};

    use axum::{Json, Router, routing::post};
    use rmcp::model::JsonRpcResponse;

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    enum JsonRpcVersion {
        #[serde(rename = "2.0")]
        Version2,
    }

    #[derive(serde::Deserialize, serde::Serialize, Debug)]
    #[serde(tag = "method", rename_all = "camelCase")]
    enum Method {
        Initialize,
        Ping,
        #[serde(untagged)]
        Unknown(String),
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    #[serde(untagged)]
    enum StringOrNumber {
        String(String),
        Number(u64),
    }

    #[derive(serde::Deserialize, serde::Serialize, Debug)]
    struct Message {
        jsonrpc: JsonRpcVersion,
        #[serde(flatten)]
        method: Method,
        /// Can be null for JSON-RPC messages but must not be null for MCP
        id: StringOrNumber,
    }

    #[derive(serde::Deserialize, serde::Serialize, Debug)]
    // #[repr(u8)]
    enum ErrorCode {}

    #[derive(serde::Deserialize, serde::Serialize, Debug)]
    struct Error {
        code: ErrorCode,
        message: Arc<str>,
        // data: Option<Arc<str>>,
    }

    #[derive(serde::Deserialize, serde::Serialize, Debug)]
    enum ProtocolVersion {
        #[serde(rename = "2025-03-26")]
        Version20250326,
    }

    #[derive(serde::Deserialize, serde::Serialize, Debug)]
    struct ServerInformation {
        name: Arc<str>,
        version: Arc<str>,
    }

    /// Empty object serialization helper
    #[derive(Debug, Default, serde::Serialize)]
    struct Empty {
        /// Empty field so that serde serializes this variant as an empty object
        #[serde(skip)]
        phantom: PhantomData<()>,
    }

    // /**
    //  * Capabilities that a server may support. Known capabilities are defined here, in this schema, but this is not a closed set: any server can define its own, additional capabilities.
    //  */
    // export interface ServerCapabilities {
    //   /**
    //    * Experimental, non-standard capabilities that the server supports.
    //    */
    //   experimental?: { [key: string]: object };
    //   /**
    //    * Present if the server supports sending log messages to the client.
    //    */
    //   logging?: object;
    //   /**
    //    * Present if the server supports argument autocompletion suggestions.
    //    */
    //   completions?: object;
    //   /**
    //    * Present if the server offers any prompt templates.
    //    */
    //   prompts?: {
    //     /**
    //      * Whether this server supports notifications for changes to the prompt list.
    //      */
    //     listChanged?: boolean;
    //   };
    //   /**
    //    * Present if the server offers any resources to read.
    //    */
    //   resources?: {
    //     /**
    //      * Whether this server supports subscribing to resource updates.
    //      */
    //     subscribe?: boolean;
    //     /**
    //      * Whether this server supports notifications for changes to the resource list.
    //      */
    //     listChanged?: boolean;
    //   };
    //   /**
    //    * Present if the server offers any tools to call.
    //    */
    //   tools?: {
    //     /**
    //      * Whether this server supports notifications for changes to the tool list.
    //      */
    //     listChanged?: boolean;
    //   };
    // }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct PromptsCapability {
        /// Whether this server supports notifications for changes to the prompt list.
        #[serde(skip_serializing_if = "Option::is_none")]
        list_changed: Option<bool>,
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ResourcesCapability {
        /// Whether this server supports subscribing to resource updates.
        #[serde(skip_serializing_if = "Option::is_none")]
        subscribe: Option<bool>,
        /// Whether this server supports notifications for changes to the resource list.
        #[serde(skip_serializing_if = "Option::is_none")]
        list_changed: Option<bool>,
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ToolsCapability {
        /// Whether this server supports notifications for changes to the tool list.
        #[serde(skip_serializing_if = "Option::is_none")]
        list_changed: Option<bool>,
    }

    /// Capabilities that a server may support. Known capabilities are defined here, in this schema, but this is not a closed set: any server can define its own, additional capabilities.
    #[derive(Debug, Default, serde::Serialize)]
    struct ServerCapabilities {
        /// Experimental, non-standard capabilities that the server supports.
        #[serde(skip_serializing_if = "Option::is_none")]
        experimental: Option<HashMap<String, serde_json::Value>>,
        /// Present if the server supports sending log messages to the client.
        #[serde(skip_serializing_if = "Option::is_none")]
        logging: Option<Empty>,
        /// Present if the server supports argument autocompletion suggestions.
        #[serde(skip_serializing_if = "Option::is_none")]
        completion: Option<Empty>,
        /// Present if the server offers any prompt templates.
        #[serde(skip_serializing_if = "Option::is_none")]
        prompts: Option<PromptsCapability>,
        /// Present if the server offers any resources to read.
        #[serde(skip_serializing_if = "Option::is_none")]
        resources: Option<ResourcesCapability>,
        /// Present if the server offers any tools to call.
        #[serde(skip_serializing_if = "Option::is_none")]
        tools: Option<ToolsCapability>,
    }

    #[derive(serde::Serialize, Debug)]
    #[serde(untagged)]
    enum SuccessResult {
        Initialize {
            #[serde(rename = "protocolVersion")]
            protocol_version: ProtocolVersion,
            capabilities: ServerCapabilities,
            #[serde(rename = "serverInfo")]
            server_information: ServerInformation,
        },
        Ping(Empty),
    }

    #[derive(serde::Serialize, Debug)]
    enum Result {
        #[serde(rename = "result")]
        Success(SuccessResult),
        #[serde(rename = "error")]
        Error(Error),
    }

    /// Represents a JSON-RPC response message
    #[derive(serde::Serialize, Debug)]
    struct Response {
        jsonrpc: JsonRpcVersion,
        id: StringOrNumber,

        #[serde(flatten)]
        result: Result,
    }

    /// Represents a JSON-RPC request message
    #[derive(serde::Deserialize, serde::Serialize, Debug)]
    #[serde(untagged)]
    enum Request {
        Message(Message),
        /// The order of messages does not matter
        BatchMessage(Vec<Message>),
    }

    enum JsonRpcIterator {
        Single(Option<Message>),
        /// The order does not matter
        Batch(Vec<Message>),
    }

    impl Iterator for JsonRpcIterator {
        type Item = Message;

        fn next(&mut self) -> Option<Self::Item> {
            match self {
                JsonRpcIterator::Single(request) => request.take(),
                // Order does not matter so we can pop from the back
                JsonRpcIterator::Batch(batch) => batch.pop(),
            }
        }
    }

    impl IntoIterator for Request {
        type Item = Message;
        type IntoIter = JsonRpcIterator;

        fn into_iter(self) -> Self::IntoIter {
            match self {
                Request::Message(request) => JsonRpcIterator::Single(Some(request)),
                Request::BatchMessage(requests) => JsonRpcIterator::Batch(requests),
            }
        }
    }

    #[axum::debug_handler]
    async fn receive_mcp_message(Json(rpc_message): Json<Request>) -> Json<Response> {
        tracing::info!("Received request: \"{rpc_message:?}\"");
        let is_batch = matches!(rpc_message, Request::BatchMessage(_));
        // Runs only once if is not a batch message
        for message in rpc_message {
            match message.method {
                Method::Initialize => {
                    if is_batch {
                        todo!(
                            "Handle protocol error that multiple messages are sent before initialization"
                        )
                    }

                    tracing::info!("Received initialize request");
                    let result = SuccessResult::Initialize {
                        protocol_version: ProtocolVersion::Version20250326,
                        capabilities: ServerCapabilities {
                            ..Default::default()
                        },
                        server_information: ServerInformation {
                            name: env!("CARGO_PKG_NAME").into(),
                            version: env!("CARGO_PKG_VERSION").into(),
                        },
                    };

                    let response = Response {
                        id: message.id,
                        jsonrpc: JsonRpcVersion::Version2,
                        result: Result::Success(result),
                    };
                    let response_json = serde_json::to_string(&response).unwrap();

                    tracing::info!("Response: {response_json}");
                    return Json(response);
                }
                Method::Ping => {
                    tracing::info!("Received Ping");

                    let result = SuccessResult::Ping(Default::default());
                    let response = Response {
                        id: message.id,
                        jsonrpc: JsonRpcVersion::Version2,
                        result: Result::Success(result),
                    };

                    let response_json = serde_json::to_string(&response).unwrap();

                    tracing::info!("Response: {response_json}");

                    return Json(response);
                }
                Method::Unknown(other) => {
                    tracing::error!("Received unknown method: {other}");
                    todo!()
                }
            }
        }

        todo!()
    }
    // async fn receive_mcp_message(request: String) {
    //     tracing::info!("Received request: \"{request:?}\"");
    // }

    pub(super) fn router() -> Router {
        Router::new().route("/mcp", post(receive_mcp_message))
    }

    #[cfg(test)]
    mod test {

        use axum::http::Method;

        use super::{Message, Request};

        #[test]
        fn can_deserialize_batch() {
            // Arrange
            let batch = r#"
                [
                    {
                        "jsonrpc": "2.0",
                        "method": "initialize",
                        "id": "as string"
                    },
                    {
                        "jsonrpc": "2.0",
                        "method": "initialize"
                        "id": 1
                    }
                ]
            "#;

            // Act
            let deserialized: Request = serde_json::from_str(batch).unwrap();
            // Assert
            assert!(matches!(deserialized, Request::BatchMessage(_)));
        }

        #[test]
        fn can_deserialize_single() {
            // Arrange
            let single = r#"
                {
                    "jsonrpc": "2.0",
                    "method": "initialize"
                }
            "#;

            // Act
            let deserialized: Request = serde_json::from_str(single).unwrap();
            // Assert
            assert!(matches!(
                deserialized,
                Request::Message(Message {
                    method: super::Method::Initialize,
                    ..
                })
            ));
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    //TODO see MUST dos: https://modelcontextprotocol.io/specification/2025-03-26/basic/transports#security-warning
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    let app = mcp::router();

    let address = "127.0.0.1:3001";
    let listener = tokio::net::TcpListener::bind(address).await?;

    tracing::info!("Listening on http://{}", address);
    axum::serve(listener, app).await?;

    Ok(())
}
