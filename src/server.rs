use std::{collections::HashMap, sync::Arc};

use crate::{
    api::{
        message::InputMessage,
        schema::{BorrowedDeserialize, Deserialize, Schema, Serialize},
    },
    server::{
        error::ThundersServerError,
        hooks::GameHooks,
        protocol::{NetworkProtocol, SessionManager},
        runtime::{GameRuntime, GameRuntimeAnyHandle, GameRuntimeHandle},
    },
};

pub mod context;
pub mod error;
pub mod hooks;
pub mod protocol;
pub mod runtime;

pub struct ThundersServer<N, S>
where
    N: NetworkProtocol,
    S: Schema,
{
    protocol: N,
    _schema: S,
    handlers: HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
    session_manager: Arc<SessionManager>,
}

impl<N, S> ThundersServer<N, S>
where
    N: NetworkProtocol,
    S: Schema + 'static,
{
    pub fn new(protocol: N, schema: S) -> Self {
        Self {
            protocol,
            _schema: schema,
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

    pub async fn run(self) -> ThundersServerResult
    where
        for<'a> InputMessage<'a>: BorrowedDeserialize<'a, S>,
    {
        let handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>> =
            Box::leak(Box::new(self.handlers));

        self.protocol.run::<S>(self.session_manager, handlers).await
    }
}

pub type ThundersServerResult = Result<(), ThundersServerError>;
