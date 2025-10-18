use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::{
    api::schema::BorrowedSerialize,
    client::reply::{Reply, ReplyManager},
};
use crate::{
    api::{
        message::{InputMessage, OutputMessage},
        schema::{Deserialize, Schema, Serialize},
    },
    client::{
        error::ThundersClientError,
        protocol::ClientProtocol,
        state::{ActiveGames, GameState, InboundAction},
    },
};

pub mod error;
pub mod protocol;
mod reply;
pub mod state;

pub type ThundersClientResult = Result<(), ThundersClientError>;

pub struct ThundersClientBuilder<P, S>
where
    S: Schema,
    P: ClientProtocol,
{
    protocol: P,
    _schema: S,
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
            _schema: schema,
            active_games: Arc::new(ActiveGames::<S> {
                current: HashMap::default(),
            }),
        }
    }

    pub fn register(mut self, type_: &'static str) -> Self {
        Arc::get_mut(&mut self.active_games)
            .expect("Should always have unique owner")
            .current
            .insert(type_, RwLock::new(HashMap::new()));
        self
    }

    pub async fn build(self) -> Result<ThundersClient<S>, ThundersClientError>
    where
        for<'a> OutputMessage<'a>: Deserialize<'a, S>,
    {
        let p_handle = self.protocol.run(Arc::clone(&self.active_games)).await?;

        Ok(ThundersClient::<S> {
            action_tx: p_handle.action_tx,
            reply_manager: p_handle.reply_manager,
            active_games: self.active_games,
        })
    }
}

pub struct ThundersClient<S: Schema> {
    action_tx: UnboundedSender<InboundAction>,
    reply_manager: Arc<ReplyManager<ThundersClientError>>,
    active_games: Arc<ActiveGames<S>>,
}

impl<S: Schema + 'static> ThundersClient<S> {
    pub async fn connect(&self, player_id: u64, expires_in: Duration) -> ThundersClientResult {
        let correlation_id = Uuid::new_v4().to_string();
        let reply = self
            .reply_manager
            .register(correlation_id.as_str(), expires_in);

        self.try_send(InputMessage::Connect {
            correlation_id: correlation_id.as_str(),
            id: player_id,
        });

        if let Ok(reply) = reply.await {
            match reply {
                Reply::Timeout => Err(ThundersClientError::NoResponse),
                Reply::Err(err) => Err(err),
                _ => Ok(()),
            }
        } else {
            Err(ThundersClientError::NoResponse)
        }
    }

    pub async fn create<G: GameState + Send + Sync + 'static>(
        &self,
        type_: &'static str,
        id: &str,
        options: G::Options,
        expires_in: Duration,
    ) -> ThundersClientResult
    where
        G::Change: for<'a> Deserialize<'a, S>,
        G::Options: Serialize<S>,
    {
        let game = G::build(&options);
        self.active_games.create(type_, id.to_string(), game)?;

        let correlation_id = Uuid::new_v4().to_string();
        let reply = self
            .reply_manager
            .register(correlation_id.as_str(), expires_in);

        let options_serialized = options.serialize();
        let options = if !options_serialized.is_empty() {
            Some(options_serialized.as_slice())
        } else {
            None
        };

        self.try_send(InputMessage::Create {
            correlation_id: correlation_id.as_str(),
            type_,
            id,
            options,
        });

        let mut should_rollback = true;
        let result = if let Ok(reply) = reply.await {
            match reply {
                Reply::Timeout => Err(ThundersClientError::NoResponse),
                Reply::Err(err) => Err(err),
                _ => {
                    should_rollback = false;
                    Ok(())
                }
            }
        } else {
            Err(ThundersClientError::NoResponse)
        };

        //TODO: Send cancellation
        if should_rollback {
            self.active_games.remove(type_, id)?;
        }

        result
    }

    pub async fn join<G: GameState + Send + Sync + 'static>(
        &self,
        type_: &'static str,
        id: &str,
        expires_in: Duration,
    ) -> ThundersClientResult
    where
        G::Change: for<'a> Deserialize<'a, S>,
    {
        let game = G::build(&G::Options::default());
        self.active_games.create(type_, id.to_string(), game)?;

        let correlation_id = Uuid::new_v4().to_string();
        let reply = self
            .reply_manager
            .register(correlation_id.as_str(), expires_in);

        self.try_send(InputMessage::Join {
            correlation_id: correlation_id.as_str(),
            type_: type_,
            id,
        });
        let mut should_rollback = true;
        let result = if let Ok(reply) = reply.await {
            match reply {
                Reply::Timeout => Err(ThundersClientError::NoResponse),
                Reply::Err(err) => Err(err),
                _ => {
                    should_rollback = false;
                    Ok(())
                }
            }
        } else {
            Err(ThundersClientError::NoResponse)
        };

        // TODO: Send cancellation
        if should_rollback {
            self.active_games.remove(type_, id)?;
        }

        result
    }

    pub fn action<G: GameState + 'static>(
        &self,
        type_: &'static str,
        id: &str,
        action: G::Action,
    ) -> ThundersClientResult
    where
        G::Action: BorrowedSerialize<S>,
    {
        self.try_send(InputMessage::Action {
            type_: type_,
            id: id,
            data: action.serialize().as_slice(),
        });

        self.active_games.action::<G>(type_, id, action)
    }

    fn try_send(&self, message: InputMessage) {
        self.action_tx
            .send(InboundAction::Raw(message.serialize()))
            .expect("Should always be consumer active if client handle alive");
    }
}

impl<S> Drop for ThundersClient<S>
where
    S: Schema,
{
    fn drop(&mut self) {
        self.action_tx
            .send(InboundAction::Stop)
            .expect("Should always be consumer active if client handle alive");
    }
}
