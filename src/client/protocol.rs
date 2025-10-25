use std::sync::Arc;

use crate::client::InternalEvent;
use crate::client::core::{ActiveGames, InboundAction};
use crate::client::reply::ReplyManager;
use crate::{
    api::{
        message::OutputMessage,
        schema::{Deserialize, Schema},
    },
    client::error::ThundersClientError,
};
use tokio::sync::mpsc::UnboundedSender;

#[cfg(feature = "ws")]
pub mod ws;

pub struct ClientProtocolHandle {
    pub(crate) action_tx: UnboundedSender<InboundAction>,
    pub(crate) event_rx: async_channel::Receiver<InternalEvent>,
    pub(crate) reply_manager: Arc<ReplyManager<ThundersClientError>>,
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
