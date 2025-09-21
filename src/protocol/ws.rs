use std::{collections::HashMap, sync::Arc};

use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Bytes, Message, Utf8Bytes},
};

use crate::{
    core::context::PlayerContext,
    protocol::{self, InputMessage, NetworkProtocol, SessionManager},
    runtime::GameRuntimeAnyHandle,
    schema::{DeSerialize, Schema, SchemaType},
};

pub struct WebSocketProtocol {
    pub addr: &'static str,
}

// Abstract all shareable behavior of message processing

impl NetworkProtocol for WebSocketProtocol {
    async fn run<S: Schema>(
        self,
        session_manager: Arc<SessionManager>,
        handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
    ) where
        InputMessage: DeSerialize<S>,
    {
        let listener = TcpListener::bind(self.addr).await.unwrap();

        loop {
            let session_manager = Arc::clone(&session_manager);
            if let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let player_cxt;
                    let ws_stream = accept_async(stream).await.unwrap();
                    let (mut write, mut read) = ws_stream.split();

                    if let Some(Ok(msg)) = read.next().await {
                        let buffer: Vec<u8> = match msg {
                            Message::Binary(bytes) => bytes.into(),
                            Message::Text(bytes) => Bytes::from(bytes).into(),
                            _ => {
                                return;
                            }
                        };

                        if let Ok(message) = <InputMessage as DeSerialize<S>>::deserialize(buffer) {
                            if let Some((cxt, mut receiver)) =
                                protocol::connect(message, session_manager.as_ref())
                            {
                                player_cxt = cxt;
                                tokio::spawn(async move {
                                    loop {
                                        let message_buffer = receiver.recv().await.unwrap();

                                        let message = match S::schema_type() {
                                            SchemaType::Text => {
                                                let result =
                                                    Utf8Bytes::try_from(message_buffer).unwrap();
                                                Message::Text(result)
                                            }

                                            SchemaType::Binary => {
                                                Message::Binary(message_buffer.into())
                                            }
                                        };
                                        if write.send(message).await.is_err() {
                                            break;
                                        }
                                    }
                                });
                            } else {
                                return;
                            }
                        } else {
                            return;
                        }
                    } else {
                        return;
                    }

                    while let Some(Ok(msg)) = read.next().await {
                        let buffer: Vec<u8> = match msg {
                            Message::Binary(bytes) => bytes.into(),
                            Message::Text(bytes) => Bytes::from(bytes).into(),
                            _ => {
                                continue;
                            }
                        };

                        if let Ok(message) = InputMessage::deserialize(buffer) {
                            protocol::process_message(
                                message,
                                &player_cxt,
                                session_manager.as_ref(),
                                handlers,
                            );
                        }
                    }

                    protocol::disconnect(player_cxt.id(), session_manager.as_ref(), handlers);
                });
            }
        }
    }
}
