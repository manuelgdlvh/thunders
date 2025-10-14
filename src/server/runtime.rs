use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    api::{
        error::ThundersError,
        schema::{Deserialize, Schema, Serialize},
    },
    server::{
        context::PlayerContext,
        hooks::{Event, GameHooks},
        protocol::SessionManager,
    },
};

pub mod sync;

pub trait GameRuntime<H, S>
where
    S: Schema,
    H: GameHooks,
    H::Delta: Serialize<S>,
    H::Options: Deserialize<S>,
    H::Action: Deserialize<S>,
{
    type Handle: GameHandle<H>;
    type Settings: Send + Sync;

    fn build(
        type_: &'static str,
        id: String,
        settings: &Self::Settings,
        session_manager: Arc<SessionManager>,
    ) -> Self;

    fn start(self, options: H::Options) -> Self::Handle;
}

// Add join and left player hooks
pub trait GameHandle<H>: Send + Sync
where
    H: GameHooks,
{
    fn event(&self, p_id: u64, event: Event<H>);
}

// Default async configurable and not with traits

pub struct GameRuntimeHandle<R, H, S>
where
    R: GameRuntime<H, S>,
    H: GameHooks,
    S: Schema,
    H::Delta: Serialize<S>,
    H::Options: Deserialize<S>,
    H::Action: Deserialize<S>,
{
    type_: &'static str,
    settings: R::Settings,
    handlers: RwLock<HashMap<String, R::Handle>>,
    session_manager: Arc<SessionManager>,
}

impl<R, H, S> GameRuntimeHandle<R, H, S>
where
    R: GameRuntime<H, S>,
    H: GameHooks,
    S: Schema,
    H::Delta: Serialize<S>,
    H::Options: Deserialize<S>,
    H::Action: Deserialize<S>,
{
    pub fn new(
        type_: &'static str,
        settings: R::Settings,
        session_manager: Arc<SessionManager>,
    ) -> Self {
        Self {
            type_,
            settings,
            handlers: RwLock::new(HashMap::new()),
            session_manager,
        }
    }

    pub fn register(&self, cxt: Arc<PlayerContext>, room_id: String, options: H::Options) {
        let runtime = R::build(
            self.type_,
            room_id.clone(),
            &self.settings,
            Arc::clone(&self.session_manager),
        );
        let r_handle = runtime.start(options);
        r_handle.event(cxt.id(), Event::Join(cxt));
        if let Ok(mut handlers) = self.handlers.write() {
            handlers.insert(room_id, r_handle);
        }
    }

    pub fn join(&self, cxt: Arc<PlayerContext>, room_id: String) {
        if let Ok(handlers) = self.handlers.read() {
            handlers.get(room_id.as_str()).inspect(|handler| {
                handler.event(cxt.id(), Event::Join(cxt));
            });
        }
    }

    pub fn leave(&self, cxt: u64, room_id: String) {
        if let Ok(handlers) = self.handlers.read() {
            handlers.get(room_id.as_str()).inspect(|handler| {
                handler.event(cxt, Event::Leave(cxt));
            });
        }
    }

    pub fn action(&self, cxt: u64, room_id: String, action: H::Action) {
        if let Ok(handlers) = self.handlers.read()
            && let Some(handler) = handlers.get(room_id.as_str())
        {
            handler.event(cxt, Event::Action(action));
        }
    }
}

pub trait GameRuntimeAnyHandle: Send + Sync {
    fn register(&self, cxt: Arc<PlayerContext>, room_id: String, options: Option<Vec<u8>>);
    fn join(&self, cxt: Arc<PlayerContext>, room_id: String);
    fn leave(&self, cxt: u64, room_id: String);
    fn action(&self, cxt: u64, room_id: String, action: Vec<u8>) -> Result<(), ThundersError>;
}

impl<R, H, S> GameRuntimeAnyHandle for GameRuntimeHandle<R, H, S>
where
    R: GameRuntime<H, S>,
    H: GameHooks,
    S: Schema,
    H::Delta: Serialize<S>,
    H::Options: Deserialize<S>,
    H::Action: Deserialize<S>,
{
    fn register(&self, cxt: Arc<PlayerContext>, room_id: String, options: Option<Vec<u8>>) {
        if let Some(options) = options {
            match <H::Options as Deserialize<S>>::deserialize(options) {
                Ok(options) => {
                    self.register(cxt, room_id, options);
                }
                Err(err) => {
                    self.session_manager.send(cxt.id(), err);
                }
            }
        } else {
            self.register(cxt, room_id, H::Options::default());
        }
    }

    fn join(&self, cxt: Arc<PlayerContext>, room_id: String) {
        self.join(cxt, room_id);
    }

    fn leave(&self, cxt: u64, room_id: String) {
        self.leave(cxt, room_id);
    }

    fn action(&self, cxt: u64, room_id: String, action: Vec<u8>) -> Result<(), ThundersError> {
        // TODO: Check if action can be borrowed to avoid allocation due to standarization of Deserialize to Borrowed.
        match <H::Action as Deserialize<S>>::deserialize(action) {
            Ok(action) => {
                self.action(cxt, room_id, action);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}
