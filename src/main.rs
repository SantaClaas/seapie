use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod mcp {
    use std::sync::Arc;

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
        // #[serde(untagged)]
        // Unknown(String),
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

    // #[derive(serde::Deserialize, serde::Serialize, Debug)]
    // #[serde(rename_all = "camelCase")]
    // enum SuccessResult {
    //     #[serde(rename = "serverInfo")]
    //     ServerCapabilities {
    //         protocol_version: ProtocolVersion,
    //         server_information: ServerInformation,
    //     },
    // }

    #[derive(serde::Deserialize, serde::Serialize, Debug)]
    enum Result {
        #[serde(rename = "result")]
        Success {
            #[serde(rename = "protocolVersion")]
            protocol_version: ProtocolVersion,
            capabilities: serde_json::Value,
            #[serde(rename = "serverInfo")]
            server_information: ServerInformation,
        },
        #[serde(rename = "error")]
        Error(Error),
    }

    /// Represents a JSON-RPC response message
    #[derive(serde::Deserialize, serde::Serialize, Debug)]
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
                    let response = Response {
                        id: message.id,
                        jsonrpc: JsonRpcVersion::Version2,
                        result: Result::Success {
                            protocol_version: ProtocolVersion::Version20250326,
                            capabilities: serde_json::Value::Object(Default::default()),
                            server_information: ServerInformation {
                                name: env!("CARGO_PKG_NAME").into(),
                                version: env!("CARGO_PKG_VERSION").into(),
                            },
                        },
                    };
                    let response_json = serde_json::to_string(&response).unwrap();

                    tracing::info!("Response: {response_json}");
                    return Json(response);
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
