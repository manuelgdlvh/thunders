use std::{any::Any, collections::HashMap, sync::RwLock};

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
    fn on_action(&mut self, action: Self::Action);
    fn on_finish(self);
}

pub trait GenericGameState<S>
where
    S: Schema,
{
    fn on_change(&mut self, change: &[u8]) -> Result<(), ThundersClientError>;

    fn on_action(&mut self, action: Box<dyn Any>) -> Result<(), ThundersClientError>;

    fn on_finished(self: Box<Self>);
}

impl<S, T> GenericGameState<S> for T
where
    S: Schema,
    T: GameState + 'static,
    T::Change: for<'a> Deserialize<'a, S> + std::fmt::Debug,
{
    fn on_change(&mut self, change: &[u8]) -> Result<(), ThundersClientError> {
        if let Ok(change) = <T::Change as Deserialize<S>>::deserialize(change) {
            self.on_change(change);
            Ok(())
        } else {
            Err(ThundersClientError::UnknownMessage)
        }
    }

    fn on_action(&mut self, action: Box<dyn Any>) -> Result<(), ThundersClientError> {
        if let Ok(action) = action.downcast::<T::Action>() {
            self.on_action(*action);
            Ok(())
        } else {
            Err(ThundersClientError::IncompatibleAction)
        }
    }
    fn on_finished(self: Box<Self>) {
        self.on_finish();
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
        message: &[u8],
    ) -> Result<(), ThundersClientError> {
        self.current
            .get(type_)
            .ok_or(ThundersClientError::RoomTypeNotFound)?
            .write()
            .expect("Should always get write lock successfully")
            .get_mut(id)
            .ok_or(ThundersClientError::RoomNotFound)?
            .as_mut()
            .on_change(message)
    }

    pub fn create<G: GameState + Send + Sync + 'static>(
        &self,
        type_: &'static str,
        id: String,
        game: G,
    ) -> Result<(), ThundersClientError>
    where
        G::Change: for<'a> Deserialize<'a, S>,
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

    pub fn action<G: GameState + 'static>(
        &self,
        type_: &'static str,
        id: &str,
        action: G::Action,
    ) -> Result<(), ThundersClientError> {
        self.current
            .get(type_)
            .ok_or(ThundersClientError::RoomTypeNotFound)?
            .write()
            .expect("Should always get write lock successfully")
            .get_mut(id)
            .ok_or(ThundersClientError::RoomNotFound)?
            .on_action(Box::new(action) as Box<dyn Any>)
    }

    pub fn remove(
        &self,
        type_: &str,
        id: &str,
    ) -> Result<Box<dyn GenericGameState<S> + Send + Sync>, ThundersClientError> {
        self.current
            .get(type_)
            .ok_or(ThundersClientError::RoomTypeNotFound)?
            .write()
            .expect("Should always get write lock successfully")
            .remove(id)
            .ok_or(ThundersClientError::RoomNotFound)
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
