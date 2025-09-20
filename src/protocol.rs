use crate::{
    runtime::GameRuntimeAnyHandle,
    schema::{DeSerialize, Schema},
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

pub mod ws;

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
        if let Ok(conns) = self.conns.read()
            && let Some(conn) = conns.get(&player_id)
        {
            conn.send(message).unwrap();
        }
    }
}

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
