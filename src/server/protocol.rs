use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
    api::{
        message::{InputMessage, OutputMessage},
        schema::{Deserialize, Schema, Serialize},
    },
    server::{
        ThundersServerResult, context::PlayerContext, error::ThundersServerError,
        runtime::GameRuntimeAnyHandle,
    },
};

#[cfg(feature = "ws")]
pub mod ws;

pub trait NetworkProtocol {
    fn run<S: Schema>(
        self,
        session_manager: Arc<SessionManager>,
        handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
    ) -> impl Future<Output = ThundersServerResult>
    where
        for<'a> InputMessage<'a>: Deserialize<'a, S>;
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
) -> Result<(Arc<PlayerContext>, UnboundedReceiver<Vec<u8>>), ThundersServerError>
where
    for<'a> InputMessage<'a>: Deserialize<'a, S>,
{
    let raw_message_ref = raw_message.as_slice();
    if let Ok(message) = <InputMessage as Deserialize<S>>::deserialize(raw_message_ref) {
        match message {
            InputMessage::Connect { correlation_id, id } => {
                let player_cxt = Arc::new(PlayerContext::new(id));
                Ok((player_cxt, session_manager.connect(correlation_id, id)))
            }
            _ => Err(ThundersServerError::MessageNotConnected),
        }
    } else {
        Err(ThundersServerError::MessageNotConnected)
    }
}

pub fn process_message<S: Schema>(
    raw_message: Vec<u8>,
    player_cxt: &Arc<PlayerContext>,
    session_manager: &SessionManager,
    handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
) where
    for<'a> InputMessage<'a>: Deserialize<'a, S>,
{
    let raw_message_ref = raw_message.as_slice();
    if let Ok(message) = <InputMessage as Deserialize<S>>::deserialize(raw_message_ref) {
        match message {
            InputMessage::Create {
                correlation_id,
                type_,
                id,
                options,
            } => {
                if let Some(handler) = handlers.get(type_) {
                    session_manager.subscribe(player_cxt.id(), type_, id);

                    // TODO: Check result to send success or not
                    handler.register(Arc::clone(player_cxt), id, options);

                    session_manager.send(
                        player_cxt.id(),
                        OutputMessage::Create {
                            correlation_id,
                            success: true,
                        },
                    );
                } else {
                    // TODO: Add correlation id to these errors
                    session_manager.send(player_cxt.id(), ThundersServerError::RoomTypeNotFound);
                }
            }
            InputMessage::Join {
                correlation_id,
                type_,
                id,
            } => {
                if let Some(handler) = handlers.get(type_) {
                    session_manager.subscribe(player_cxt.id(), type_, id);
                    handler.join(Arc::clone(player_cxt), id);

                    session_manager.send(
                        player_cxt.id(),
                        OutputMessage::Join {
                            correlation_id,
                            success: true,
                        },
                    );
                } else {
                    session_manager.send(player_cxt.id(), ThundersServerError::RoomTypeNotFound);
                }
            }
            InputMessage::Action { type_, id, data } => {
                if let Some(handler) = handlers.get(type_) {
                    let _ = handler.action(player_cxt.id(), id, data);
                }
            }
            _ => {}
        }
    } else {
        session_manager.send(player_cxt.id(), ThundersServerError::DeserializationFailure);
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
    pub fn connect(&self, correlation_id: &str, player_id: u64) -> UnboundedReceiver<Vec<u8>> {
        let (tx, rx) = mpsc::unbounded_channel::<Vec<u8>>();

        tx.send(
            OutputMessage::Connect {
                correlation_id,
                success: true,
            }
            .serialize(),
        )
        .unwrap();
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.insert(player_id, tx);
        }

        if let Ok(mut subscriptions) = self.subscriptions.write() {
            subscriptions.insert(player_id, HashMap::default());
        }

        rx
    }

    pub fn subscribe(&self, player_id: u64, type_: &str, id: &str) {
        if let Ok(mut subscriptions) = self.subscriptions.write() {
            let subscriptions = subscriptions
                .get_mut(&player_id)
                .expect("Player subscriptions should always exists if connected");
            subscriptions
                .entry(type_.to_string())
                .or_insert(Default::default())
                .push(id.to_string());
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

    pub fn send<'a>(&self, player_id: u64, message: impl Into<OutputMessage<'a>>) {
        if let Ok(sessions) = self.sessions.read()
            && let Some(session) = sessions.get(&player_id)
        {
            let _ = session.send(message.into().serialize());
        }
    }

    pub fn send_all<'a>(
        &self,
        player_ids: impl Iterator<Item = &'a u64>,
        message: impl Into<OutputMessage<'a>>,
    ) {
        let raw_message = message.into().serialize();

        for p_id in player_ids {
            if let Ok(sessions) = self.sessions.read()
                && let Some(session) = sessions.get(p_id)
            {
                let _ = session.send(raw_message.clone());
            }
        }
    }
}
