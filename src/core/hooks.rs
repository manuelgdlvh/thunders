// Send actions and update the state. Each tick returns the delta.

use std::{collections::HashMap, sync::Arc};

use crate::core::context::PlayerContext;

pub trait GameHooks: Send + 'static {
    type Delta: Send;
    type Action: Send;
    type Options: Default;

    fn build(options: Self::Options) -> Self;
    fn diff(
        &self,
        player_cxts: &HashMap<u64, Arc<PlayerContext>>,
        actions: &[(u64, Self::Action)],
    ) -> Vec<Diff<Self::Delta>>;

    fn join(&self, player_cxt: &PlayerContext) -> Option<Vec<Diff<Self::Delta>>>;
    fn leave(&self, player_cxt: &PlayerContext) -> Option<Diff<Self::Delta>>;
    fn update(&mut self, actions: Vec<(u64, Self::Action)>);
    fn finish(&self) -> (bool, Option<Diff<Self::Delta>>);
}

pub enum Diff<D> {
    All { delta: D },
    Target { ids: Vec<u64>, delta: D },
}

pub enum Event<H>
where
    H: GameHooks,
{
    Action(H::Action),
    Join(Arc<PlayerContext>),
    Leave(u64),
}

pub struct DiffNotification<'a> {
    pub type_: &'static str,
    pub id: &'a str,
    pub data: Vec<u8>,
}

impl<'a> DiffNotification<'a> {
    pub fn new(type_: &'static str, id: &'a str, data: Vec<u8>) -> Self {
        Self { type_, id, data }
    }
}
