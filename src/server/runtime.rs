use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    api::{
        error::ThundersError,
        schema::{Deserialize, Schema, Serialize},
    },
    server::{context::PlayerContext, hooks::GameHooks, protocol::SessionManager},
};

pub mod sync;

#[derive(Debug)]
pub enum RuntimeAction<H>
where
    H: GameHooks,
{
    Action(H::Action),
    Join(Arc<PlayerContext>),
    Leave(u64),
}

pub trait GameRuntime<H, S>
where
    S: Schema,
    H: GameHooks,
    H::Delta: Serialize<S>,
    H::Options: for<'a> Deserialize<'a, S>,
    H::Action: for<'a> Deserialize<'a, S>,
{
    type Handle: GameHandle<H>;
    type Settings: Send + Sync;

    fn build(
        type_: &'static str,
        id: String,
        hooks: H,
        settings: &Self::Settings,
        session_manager: Arc<SessionManager>,
    ) -> Self;

    fn start(self) -> Self::Handle;
}

// Add join and left player hooks
pub trait GameHandle<H>: Send + Sync
where
    H: GameHooks,
{
    fn send(&self, p_id: u64, action: RuntimeAction<H>);
}

// Default async configurable and not with traits

pub struct GameRuntimeHandle<R, H, S>
where
    R: GameRuntime<H, S>,
    H: GameHooks,
    S: Schema,
    H::Delta: Serialize<S>,
    H::Options: for<'a> Deserialize<'a, S>,
    H::Action: for<'a> Deserialize<'a, S>,
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
    H::Options: for<'a> Deserialize<'a, S>,
    H::Action: for<'a> Deserialize<'a, S>,
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
            H::build(options),
            &self.settings,
            Arc::clone(&self.session_manager),
        );
        let r_handle = runtime.start();
        r_handle.send(cxt.id(), RuntimeAction::Join(cxt));
        if let Ok(mut handlers) = self.handlers.write() {
            handlers.insert(room_id, r_handle);
        }
    }

    pub fn join(&self, cxt: Arc<PlayerContext>, room_id: String) {
        if let Ok(handlers) = self.handlers.read() {
            handlers.get(room_id.as_str()).inspect(|handler| {
                handler.send(cxt.id(), RuntimeAction::Join(cxt));
            });
        }
    }

    pub fn leave(&self, cxt: u64, room_id: String) {
        if let Ok(handlers) = self.handlers.read() {
            handlers.get(room_id.as_str()).inspect(|handler| {
                handler.send(cxt, RuntimeAction::Leave(cxt));
            });
        }
    }

    pub fn action(&self, cxt: u64, room_id: String, action: H::Action) {
        if let Ok(handlers) = self.handlers.read()
            && let Some(handler) = handlers.get(room_id.as_str())
        {
            handler.send(cxt, RuntimeAction::Action(action));
        }
    }
}

pub trait GameRuntimeAnyHandle: Send + Sync {
    fn register(&self, cxt: Arc<PlayerContext>, room_id: &str, options: Option<&[u8]>);
    fn join(&self, cxt: Arc<PlayerContext>, room_id: &str);
    fn leave(&self, cxt: u64, room_id: String);
    fn action(&self, cxt: u64, room_id: &str, action: &[u8]) -> Result<(), ThundersError>;
}

impl<R, H, S> GameRuntimeAnyHandle for GameRuntimeHandle<R, H, S>
where
    R: GameRuntime<H, S>,
    H: GameHooks,
    S: Schema,
    H::Delta: Serialize<S>,
    H::Options: for<'a> Deserialize<'a, S>,
    H::Action: for<'a> Deserialize<'a, S>,
{
    fn register(&self, cxt: Arc<PlayerContext>, room_id: &str, options: Option<&[u8]>) {
        if let Some(options) = options {
            match <H::Options as Deserialize<S>>::deserialize(options) {
                Ok(options) => {
                    self.register(cxt, room_id.to_string(), options);
                }
                Err(err) => {
                    self.session_manager.send(cxt.id(), err);
                }
            }
        } else {
            self.register(cxt, room_id.to_string(), H::Options::default());
        }
    }

    fn join(&self, cxt: Arc<PlayerContext>, room_id: &str) {
        self.join(cxt, room_id.to_string());
    }

    fn leave(&self, cxt: u64, room_id: String) {
        self.leave(cxt, room_id);
    }

    fn action(&self, cxt: u64, room_id: &str, action: &[u8]) -> Result<(), ThundersError> {
        match <H::Action as Deserialize<S>>::deserialize(action) {
            Ok(action) => {
                self.action(cxt, room_id.to_string(), action);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}
