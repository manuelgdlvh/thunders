use core::panic::PanicMessage;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use futures::{SinkExt, StreamExt};
use tokio::{
    net::TcpListener,
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Bytes, Message, Utf8Bytes},
};

use crate::{
    runtime::GameRuntimeAnyHandle,
    schema::{DeSerialize, Schema, SchemaType},
};

// Abstract to receive deserializer to use whatever serialization schema wanted. Provide default implementation for json. Also new protocols could be implemented
pub trait NetworkProtocol {
    fn run<S: Schema>(
        self,
        session_manager: Arc<SessionManager>,
        handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
    ) -> impl Future<Output = ()>
    where
        InputMessage: DeSerialize<S>;
}

pub struct WebSocketProtocol {
    pub addr: &'static str,
}

impl NetworkProtocol for WebSocketProtocol {
    async fn run<S: Schema>(
        self,
        session_manager: Arc<SessionManager>,
        handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
    ) where
        InputMessage: DeSerialize<S>,
    {
        println!("Starting network protocol...");

        let listener = TcpListener::bind(self.addr).await.unwrap();

        loop {
            let session_manager = Arc::clone(&session_manager);
            if let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(async move {
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
                            match message {
                                InputMessage::Connect { id } => {
                                    let mut notification_channel = session_manager.connect(id);

                                    tokio::spawn(async move {
                                        loop {
                                            let message_buffer =
                                                notification_channel.recv().await.unwrap();

                                            let message = match S::schema_type() {
                                                SchemaType::Text => {
                                                    let result =
                                                        Utf8Bytes::try_from(message_buffer)
                                                            .unwrap();
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
                                }
                                _ => {
                                    return;
                                }
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
                            match message {
                                InputMessage::Create { type_, id, options } => {
                                    if let Some(handler) = handlers.get(type_.as_str()) {
                                        handler.register(id, options);
                                    }
                                }
                                InputMessage::Join { type_, id } => {}
                                InputMessage::Action { type_, id, data } => {
                                    handlers
                                        .get(type_.as_str())
                                        .map(|handler| handler.action(id, data));
                                }
                                _ => {}
                            }
                        }
                    }
                });
            }
        }
    }
}

// Abstract network protocol, deserialization schema and notifier
// Move shared types(requests, error messages, etc...) and traits to protocol module and all related with ws to ws module.
#[derive(Default)]
pub struct SessionManager {
    conns: RwLock<HashMap<u64, UnboundedSender<Vec<u8>>>>,
}

impl SessionManager {
    pub fn connect(&self, player_id: u64) -> UnboundedReceiver<Vec<u8>> {
        let (tx, rx) = mpsc::unbounded_channel::<Vec<u8>>();
        if let Ok(mut conns) = self.conns.write() {
            conns.insert(player_id, tx);
        }
        rx
    }

    pub fn send(&self, player_id: u64, message: Vec<u8>) {
        if let Ok(conns) = self.conns.read() {
            if let Some(conn) = conns.get(&player_id) {
                conn.send(message).unwrap();
            }
        }
    }
}

// Schema
// Change io Error to custom Error type
pub enum InputMessage {
    Connect {
        id: u64,
    },
    Create {
        type_: String,
        id: String,
        options: Option<Vec<u8>>,
    },
    Join {
        type_: String,
        id: String,
    },
    Action {
        type_: String,
        id: String,
        data: Vec<u8>,
    },
}

// Abstract over the serialization schema used
