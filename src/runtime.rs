use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    core::hooks::GameHooks,
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
    fn action(&self, action: H::Action);
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

    pub fn register(&self, id: String, options: H::Options) {
        let runtime = R::start(options, Arc::clone(&self.session_manager));
        if let Ok(mut handlers) = self.handlers.write() {
            handlers.insert(id, runtime);
        }
    }

    pub fn action(&self, id: String, action: H::Action) {
        if let Ok(handlers) = self.handlers.read() {
            if let Some(handler) = handlers.get(id.as_str()) {
                handler.action(action);
            }
        }
    }
}

pub trait GameRuntimeAnyHandle: Send + Sync {
    fn register(&self, id: String, options: Option<Vec<u8>>);
    fn action(&self, id: String, action: Vec<u8>);
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
    fn register(&self, id: String, options: Option<Vec<u8>>) {
        if let Some(options) = options {
            let result = <H::Options as DeSerialize<S>>::deserialize(options).unwrap();
            self.register(id, result);
        } else {
            self.register(id, H::Options::default());
        }
    }

    fn action(&self, id: String, action: Vec<u8>) {
        let action = <H::Action as DeSerialize<S>>::deserialize(action).unwrap();
        self.action(id, action);
    }
}
