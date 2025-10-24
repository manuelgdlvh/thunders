use std::{collections::HashMap, sync::Arc};

use crate::{api::message::OutputMessage, server::context::PlayerContext};

pub trait GameHooks: Send + 'static {
    type Delta: Send;
    type Action: Send + std::fmt::Debug;
    type Options: Default + std::fmt::Debug;

    fn build(host_id: u64, options: Self::Options) -> Self;

    fn tick(
        &mut self,
        player_cxts: &HashMap<u64, Arc<PlayerContext>>,
        actions: Vec<(u64, Self::Action)>,
    ) -> Option<Vec<Diff<Self::Delta>>>;

    fn join(&mut self, player_cxt: &PlayerContext) -> Option<Vec<Diff<Self::Delta>>>;
    fn leave(&mut self, player_cxt: &PlayerContext) -> Option<Diff<Self::Delta>>;
    fn finish(&self) -> (bool, Option<Diff<Self::Delta>>);
}

pub enum Diff<D> {
    All { delta: D },
    Target { ids: Vec<u64>, delta: D },
}

#[derive(Debug)]
pub enum Event<H>
where
    H: GameHooks,
{
    Action(H::Action),
    Join(Arc<PlayerContext>),
    Leave(u64),
}

#[derive(Debug)]
pub struct DiffNotification<'a> {
    pub type_: &'static str,
    pub id: &'a str,
    pub finished: bool,
    pub data: Vec<u8>,
}

impl<'a> DiffNotification<'a> {
    pub fn new(type_: &'static str, id: &'a str, data: Vec<u8>) -> Self {
        Self {
            type_,
            id,
            finished: false,
            data,
        }
    }

    pub fn finish(type_: &'static str, id: &'a str) -> Self {
        Self {
            type_,
            id,
            finished: true,
            data: vec![],
        }
    }
}

impl<'a> From<&'a DiffNotification<'a>> for OutputMessage<'a> {
    fn from(val: &'a DiffNotification<'a>) -> Self {
        OutputMessage::Diff {
            type_: val.type_,
            id: val.id,
            finished: val.finished,
            data: val.data.as_slice(),
        }
    }
}
