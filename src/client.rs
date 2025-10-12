use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use crate::{
    api::{
        message::{InputMessage, OutputMessage},
        schema::{Deserialize, Schema, Serialize},
    },
    client::{
        error::ThundersClientError,
        protocol::{ClientProtocol, ClientProtocolHandle},
        state::{ActiveGames, GameState, InboundAction},
    },
};

pub mod error;
pub mod protocol;
pub mod state;

pub struct ThundersClientBuilder<P, S>
where
    S: Schema,
    P: ClientProtocol,
{
    protocol: P,
    schema: S,
    active_games: Arc<ActiveGames<S>>,
}

impl<P, S> ThundersClientBuilder<P, S>
where
    S: Schema + 'static,
    P: ClientProtocol,
{
    pub fn new(protocol: P, schema: S) -> Self {
        Self {
            protocol,
            schema,
            active_games: Arc::new(ActiveGames::<S> {
                current: HashMap::default(),
            }),
        }
    }

    pub fn register(mut self, type_: &'static str) -> Self {
        Arc::get_mut(&mut self.active_games)
            .expect("")
            .current
            .insert(type_, RwLock::new(HashMap::new()));
        self
    }

    pub async fn build(self) -> Result<ThundersClient<S>, ThundersClientError>
    where
        for<'a> OutputMessage<'a>: Deserialize<S>,
    {
        let p_handle = self.protocol.run(Arc::clone(&self.active_games)).await?;

        Ok(ThundersClient::<S> {
            p_handle,
            active_games: self.active_games,
        })
    }
}

pub struct ThundersClient<S: Schema> {
    p_handle: ClientProtocolHandle,
    active_games: Arc<ActiveGames<S>>,
}

impl<S: Schema + 'static> ThundersClient<S> {
    // Add awaitable callback with timeout to know if successfully created or joined

    pub async fn connect(&mut self, id: u64) -> Result<(), ThundersClientError> {
        let correlation_id = format!("{:?}", Instant::now());
        let awaitable = self
            .p_handle
            .reply_manager
            .register(correlation_id.clone(), Duration::from_secs(5));

        self.try_send(InputMessage::Connect { correlation_id, id });

        if let Ok(reply) = awaitable.await {
            match reply {
                reply_maybe::Reply::Timeout => Err(ThundersClientError::NoResponse),
                reply_maybe::Reply::Err(err) => Err(err),
                _ => Ok(()),
            }
        } else {
            Err(ThundersClientError::NoResponse)
        }
    }

    pub async fn create<G: GameState + Send + Sync + 'static>(
        &self,
        type_: &'static str,
        id: String,
        game: G,
    ) where
        G::Change: Deserialize<S>,
    {
        let _ = self.active_games.create(type_, id.clone(), game);
        self.try_send(InputMessage::Create {
            type_: type_.to_string(),
            id,
            options: None,
        });
    }

    pub fn join<G: GameState + Send + Sync + 'static>(
        &self,
        type_: &'static str,
        id: String,
        game: G,
    ) where
        G::Change: Deserialize<S>,
    {
        // TODO: Change this
        let _ = self.active_games.create(type_, id.clone(), game);
        self.try_send(InputMessage::Join {
            type_: type_.to_string(),
            id,
        });
    }

    pub fn action<G: GameState + 'static>(&self, type_: &'static str, id: String, action: G::Action)
    where
        G::Action: Serialize<S>,
    {
        self.try_send(InputMessage::Action {
            type_: type_.to_string(),
            id,
            data: action.serialize(),
        });
    }

    fn try_send(&self, message: InputMessage) {
        self.p_handle
            .sender
            .send(InboundAction::Raw(message.serialize()))
            .unwrap();
    }
}

impl<S> Drop for ThundersClient<S>
where
    S: Schema,
{
    fn drop(&mut self) {
        self.p_handle.sender.send(InboundAction::Stop).unwrap();
    }
}
