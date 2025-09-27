use crate::{
    core::context::PlayerContext,
    runtime::GameRuntimeAnyHandle,
    schema::{Deserialize, Schema, Serialize},
};
use std::{
    collections::HashMap,
    error::Error,
    fmt::Display,
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
    ) -> impl Future<Output = Result<(), ThundersError>>
    where
        InputMessage: Deserialize<S>;
}

pub fn disconnect(
    p_id: u64,
    session_manager: &SessionManager,
    handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
) {
    if let Some(subscriptions) = session_manager.unsubscribe_all(p_id) {
        for (room_type, room_ids) in subscriptions {
            let handler = handlers
                .get(room_type.as_str())
                .expect("Should always exist handler type if previously registered");
            for room_id in room_ids {
                handler.leave(p_id, room_id);
            }
        }
    }
}

pub fn connect<S: Schema>(
    raw_message: Vec<u8>,
    session_manager: &SessionManager,
) -> Result<(Arc<PlayerContext>, UnboundedReceiver<Vec<u8>>), ThundersError>
where
    InputMessage: Deserialize<S>,
{
    if let Ok(message) = <InputMessage as Deserialize<S>>::deserialize(raw_message) {
        match message {
            InputMessage::Connect { id } => {
                let player_cxt = Arc::new(PlayerContext::new(id));
                Ok((player_cxt, session_manager.connect(id)))
            }
            _ => Err(ThundersError::MessageNotConnected),
        }
    } else {
        Err(ThundersError::MessageNotConnected)
    }
}

pub fn process_message<S: Schema>(
    raw_message: Vec<u8>,
    player_cxt: &Arc<PlayerContext>,
    session_manager: &SessionManager,
    handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
) where
    InputMessage: Deserialize<S>,
{
    if let Ok(message) = <InputMessage as Deserialize<S>>::deserialize(raw_message) {
        match message {
            InputMessage::Create { type_, id, options } => {
                if let Some(handler) = handlers.get(type_.as_str()) {
                    session_manager.subscribe(player_cxt.id(), type_, id.clone());
                    handler.register(Arc::clone(&player_cxt), id, options);
                }
            }
            InputMessage::Join { type_, id } => {
                if let Some(handler) = handlers.get(type_.as_str()) {
                    session_manager.subscribe(player_cxt.id(), type_, id.clone());
                    handler.join(Arc::clone(&player_cxt), id);
                }
            }
            InputMessage::Action { type_, id, data } => {
                if let Some(handler) = handlers.get(type_.as_str()) {
                    let _ = handler.action(player_cxt.id(), id, data);
                }
            }
            _ => {}
        }
    } else {
        session_manager.send(
            player_cxt.id(),
            ThundersError::DeserializationFailure.serialize(),
        );
    }
}

// Abstract network protocol, deserialization schema and notifier
// Move shared types(requests, error messages, etc...) and traits to protocol module and all related with ws to ws module.
#[derive(Default)]
pub struct SessionManager {
    sessions: RwLock<HashMap<u64, UnboundedSender<Vec<u8>>>>,
    subscriptions: RwLock<HashMap<u64, HashMap<String, Vec<String>>>>,
}

impl SessionManager {
    pub fn connect(&self, player_id: u64) -> UnboundedReceiver<Vec<u8>> {
        let (tx, rx) = mpsc::unbounded_channel::<Vec<u8>>();
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.insert(player_id, tx);
        }

        if let Ok(mut subscriptions) = self.subscriptions.write() {
            subscriptions.insert(player_id, HashMap::default());
        }

        rx
    }

    pub fn subscribe(&self, player_id: u64, type_: String, id: String) {
        if let Ok(mut subscriptions) = self.subscriptions.write() {
            let subscriptions = subscriptions
                .get_mut(&player_id)
                .expect("Player subscriptions should always exists if connected");
            subscriptions
                .entry(type_)
                .or_insert(Default::default())
                .push(id);
        }
    }

    pub fn unsubscribe(&self, player_id: u64, type_: String, id: String) {
        todo!()
    }

    pub fn unsubscribe_all(&self, player_id: u64) -> Option<HashMap<String, Vec<String>>> {
        self.subscriptions
            .write()
            .expect("Lock should never be poisoned")
            .remove(&player_id)
    }

    pub fn send(&self, player_id: u64, message: Vec<u8>) {
        if let Ok(sessions) = self.sessions.read()
            && let Some(session) = sessions.get(&player_id)
        {
            let _ = session.send(message);
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

#[derive(Debug)]
pub enum ThundersError {
    StartFailure,
    MessageNotConnected,
    ConnectionFailure,
    InvalidInput,
    DeserializationFailure,
}

impl Display for ThundersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl Error for ThundersError {}
