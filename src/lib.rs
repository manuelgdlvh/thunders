use std::{collections::HashMap, sync::Arc};

use crate::{
    core::hooks::GameHooks,
    protocol::{InputMessage, NetworkProtocol, SessionManager},
    runtime::{GameRuntime, GameRuntimeAnyHandle, GameRuntimeHandle},
    schema::{Deserialize, Schema, Serialize},
};

pub mod core;
pub mod protocol;
pub mod runtime;
pub mod schema;

pub struct MultiPlayer<N, S>
where
    N: NetworkProtocol,
    S: Schema,
{
    protocol: N,
    schema: S,
    handlers: HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
    session_manager: Arc<SessionManager>,
}

impl<N, S> MultiPlayer<N, S>
where
    N: NetworkProtocol,
    S: Schema + 'static,
{
    pub fn new(protocol: N, schema: S) -> Self {
        Self {
            protocol,
            schema,
            handlers: Default::default(),
            session_manager: Arc::new(SessionManager::default()),
        }
    }

    pub fn register<R: GameRuntime<H, S> + 'static, H: GameHooks>(
        mut self,
        type_: &'static str,
        settings: R::Settings,
    ) -> Self
    where
        H::Delta: Serialize<S>,
        H::Options: Deserialize<S>,
        H::Action: Deserialize<S>,
    {
        self.handlers.insert(
            type_,
            Box::new(GameRuntimeHandle::<R, H, S>::new(
                type_,
                settings,
                Arc::clone(&self.session_manager),
            )),
        );
        self
    }

    pub async fn run(self)
    where
        InputMessage: Deserialize<S>,
    {
        let handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>> =
            Box::leak(Box::new(self.handlers));

        self.protocol.run::<S>(self.session_manager, handlers).await;
    }
}
