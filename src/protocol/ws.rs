use std::{collections::HashMap, sync::Arc};

use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Bytes, Message, Utf8Bytes},
};

use crate::{
    protocol::{self, InputMessage, NetworkProtocol, SessionManager, ThundersError},
    runtime::GameRuntimeAnyHandle,
    schema::{Deserialize, Schema, SchemaType, Serialize},
};

pub struct WebSocketProtocol {
    addr: String,
    port: u16,
}

impl WebSocketProtocol {
    pub fn new(addr: impl Into<String>, port: u16) -> Self {
        Self {
            addr: addr.into(),
            port,
        }
    }
}

// Abstract all shareable behavior of message processing

impl NetworkProtocol for WebSocketProtocol {
    async fn run<S: Schema>(
        self,
        session_manager: Arc<SessionManager>,
        handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
    ) -> Result<(), ThundersError>
    where
        InputMessage: Deserialize<S>,
    {
        let listener = TcpListener::bind(format!("{}:{}", self.addr, self.port).as_str())
            .await
            .map_err(|_| ThundersError::StartFailure)?;

        loop {
            let session_manager = Arc::clone(&session_manager);
            if let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let player_cxt;
                    let ws_stream = match accept_async(stream).await {
                        Ok(ws_stream) => ws_stream,
                        Err(_) => {
                            return;
                        }
                    };
                    let (mut write, mut read) = ws_stream.split();

                    if let Some(Ok(msg)) = read.next().await {
                        let raw_message: Vec<u8> = message_into_bytes(msg);
                        match protocol::connect(raw_message, session_manager.as_ref()) {
                            Ok((cxt, mut receiver)) => {
                                player_cxt = cxt;
                                tokio::spawn(async move {
                                    loop {
                                        match receiver.recv().await {
                                            Some(raw_message) => {
                                                if write
                                                    .send(bytes_into_message::<S>(raw_message))
                                                    .await
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                            }

                                            None => {
                                                break;
                                            }
                                        };
                                    }
                                });
                            }
                            Err(err) => {
                                let _ = write.send(bytes_into_message::<S>(err.serialize())).await;
                                return;
                            }
                        }
                    } else {
                        let _ = write
                            .send(bytes_into_message::<S>(
                                ThundersError::MessageNotConnected.serialize(),
                            ))
                            .await;
                        return;
                    }

                    while let Some(Ok(msg)) = read.next().await {
                        let raw_message: Vec<u8> = message_into_bytes(msg);
                        protocol::process_message(
                            raw_message,
                            &player_cxt,
                            session_manager.as_ref(),
                            handlers,
                        );
                    }

                    protocol::disconnect(player_cxt.id(), session_manager.as_ref(), handlers);
                });
            } else {
                // Check tcp stream closed error
                break;
            }
        }

        Ok(())
    }
}

fn bytes_into_message<S: Schema>(raw_message: Vec<u8>) -> Message {
    match S::schema_type() {
        SchemaType::Text => {
            let result =
                Utf8Bytes::try_from(raw_message).expect("Should always be parsable to utf-8 bytes");
            Message::Text(result)
        }

        SchemaType::Binary => Message::Binary(raw_message.into()),
    }
}

fn message_into_bytes(message: Message) -> Vec<u8> {
    match message {
        Message::Binary(bytes) => bytes.into(),
        Message::Text(bytes) => Bytes::from(bytes).into(),
        _ => {
            vec![]
        }
    }
}
