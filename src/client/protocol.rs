use std::sync::{Arc, atomic::AtomicBool};

use crate::client::reply::ReplyManager;
use crate::{
    api::{
        message::OutputMessage,
        schema::{Deserialize, Schema},
    },
    client::{
        error::ThundersClientError,
        state::{ActiveGames, InboundAction},
    },
};
use tokio::sync::mpsc::UnboundedSender;

#[cfg(feature = "ws")]
pub mod ws;

pub struct ClientProtocolHandle {
    pub sender: UnboundedSender<InboundAction>,
    pub reply_manager: Arc<ReplyManager<(), ThundersClientError>>,
    pub running: Arc<AtomicBool>,
}

pub trait ClientProtocol {
    fn run<S>(
        self,
        active_games: Arc<ActiveGames<S>>,
    ) -> impl Future<Output = Result<ClientProtocolHandle, ThundersClientError>>
    where
        S: Schema + 'static,
        for<'a> OutputMessage<'a>: Deserialize<'a, S>;
}
