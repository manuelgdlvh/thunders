use std::{collections::HashMap, sync::RwLock};

use tokio::sync::mpsc::UnboundedSender;

use crate::{
    api::schema::{Deserialize, Schema},
    client::error::ThundersClientError,
};

pub trait GameState {
    type Change: std::fmt::Debug;
    type Action;
    type Options: Default;

    fn build(options: &Self::Options) -> Self;

    fn on_change(&mut self, change: Self::Change);
}

pub trait GenericGameState<S>
where
    S: Schema,
{
    fn on_change(&mut self, change: Vec<u8>) -> Result<(), ThundersClientError>;
}

impl<S, T> GenericGameState<S> for T
where
    S: Schema,
    T: GameState,
    T::Change: Deserialize<S> + std::fmt::Debug,
{
    fn on_change(&mut self, change: Vec<u8>) -> Result<(), ThundersClientError> {
        if let Ok(change) = <T::Change as Deserialize<S>>::deserialize(change) {
            self.on_change(change);
            Ok(())
        } else {
            Err(ThundersClientError::UnknownMessage)
        }
    }
}

pub struct ActiveGames<S: Schema> {
    pub current:
        HashMap<&'static str, RwLock<HashMap<String, Box<dyn GenericGameState<S> + Send + Sync>>>>,
}

impl<S: Schema> ActiveGames<S> {
    pub fn route_message(
        &self,
        type_: &str,
        id: &str,
        message: Vec<u8>,
    ) -> Result<(), ThundersClientError> {
        Ok(self
            .current
            .get(type_)
            .ok_or(ThundersClientError::RoomTypeNotFound)?
            .write()
            .expect("Should always get write lock successfully")
            .get_mut(id)
            .ok_or(ThundersClientError::RoomNotFound)?
            .as_mut()
            .on_change(message)?)
    }

    pub fn create<G: GameState + Send + Sync + 'static>(
        &self,
        type_: &'static str,
        id: String,
        game: G,
    ) -> Result<(), ThundersClientError>
    where
        G::Change: Deserialize<S>,
    {
        self.current
            .get(type_)
            .ok_or(ThundersClientError::RoomTypeNotFound)?
            .write()
            .expect("Should always get write lock successfully")
            .insert(
                id,
                Box::new(game) as Box<dyn GenericGameState<S> + Send + Sync>,
            );

        Ok(())
    }

    pub fn remove(&self, type_: &'static str, id: &str) -> Result<(), ThundersClientError> {
        self.current
            .get(type_)
            .ok_or(ThundersClientError::RoomTypeNotFound)?
            .write()
            .expect("Should always get write lock successfully")
            .remove(id);
        Ok(())
    }
}

pub enum InboundAction {
    Raw(Vec<u8>),
    Stop,
}

pub struct GameStateRuntime<S>
where
    S: GameState,
{
    state: S,
    action_rx: tokio::sync::mpsc::UnboundedReceiver<S::Change>,
}

impl<S> GameStateRuntime<S>
where
    S: GameState,
{
    pub fn new(state: S) -> (Self, UnboundedSender<S::Change>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<S::Change>();
        let self_ = Self {
            state,
            action_rx: rx,
        };

        (self_, tx)
    }

    pub async fn run(mut self) {
        // Add stop flag
        while let Some(change) = self.action_rx.recv().await {
            self.state.on_change(change);
        }
    }
}
