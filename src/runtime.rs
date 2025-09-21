use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    core::{
        context::PlayerContext,
        hooks::{Event, GameHooks},
    },
    protocol::SessionManager,
    schema::{DeSerialize, Schema},
};

pub mod sync;

pub trait GameRuntime<H, S>
where
    S: Schema,
    H: GameHooks,
    H::Delta: DeSerialize<S>,
    H::Options: DeSerialize<S>,
    H::Action: DeSerialize<S>,
{
    type Handle: GameHandle<H>;

    fn start(options: H::Options, session_manager: Arc<SessionManager>) -> Self::Handle;
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
    H::Delta: DeSerialize<S>,
    H::Options: DeSerialize<S>,
    H::Action: DeSerialize<S>,
{
    handlers: RwLock<HashMap<String, R::Handle>>,
    session_manager: Arc<SessionManager>,
}

impl<R, H, S> GameRuntimeHandle<R, H, S>
where
    R: GameRuntime<H, S>,
    H: GameHooks,
    S: Schema,
    H::Delta: DeSerialize<S>,
    H::Options: DeSerialize<S>,
    H::Action: DeSerialize<S>,
{
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            session_manager,
        }
    }

    pub fn register(&self, cxt: Arc<PlayerContext>, room_id: String, options: H::Options) {
        let r_handle = R::start(options, Arc::clone(&self.session_manager));
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
    fn action(&self, cxt: u64, room_id: String, action: Vec<u8>) -> Result<(), std::io::Error>;
}

impl<R, H, S> GameRuntimeAnyHandle for GameRuntimeHandle<R, H, S>
where
    R: GameRuntime<H, S>,
    H: GameHooks,
    S: Schema,
    H::Delta: DeSerialize<S>,
    H::Options: DeSerialize<S>,
    H::Action: DeSerialize<S>,
{
    fn register(&self, cxt: Arc<PlayerContext>, room_id: String, options: Option<Vec<u8>>) {
        if let Some(options) = options {
            let result = <H::Options as DeSerialize<S>>::deserialize(options).unwrap();
            self.register(cxt, room_id, result);
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

    fn action(&self, cxt: u64, room_id: String, action: Vec<u8>) -> Result<(), std::io::Error> {
        match <H::Action as DeSerialize<S>>::deserialize(action) {
            Ok(action) => {
                self.action(cxt, room_id, action);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}
