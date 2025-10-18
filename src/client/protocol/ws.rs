use futures::{SinkExt, StreamExt};
use std::sync::Arc;

use tokio_tungstenite::{
    connect_async,
    tungstenite::{Bytes, Message, client::IntoClientRequest},
};

use crate::client::reply::ReplyManager;
use crate::{
    api::{
        message::OutputMessage,
        schema::{Deserialize, Schema},
    },
    client::{
        error::ThundersClientError,
        protocol::{ClientProtocol, ClientProtocolHandle},
        state::{ActiveGames, InboundAction},
    },
};

pub struct WebSocketClientProtocol {
    pub addr: String,
    pub port: u16,
}

impl WebSocketClientProtocol {
    pub fn new(addr: String, port: u16) -> Self {
        Self { addr, port }
    }
}
impl ClientProtocol for WebSocketClientProtocol {
    async fn run<S>(
        self,
        active_games: Arc<ActiveGames<S>>,
    ) -> Result<ClientProtocolHandle, ThundersClientError>
    where
        S: Schema + 'static,
        for<'a> OutputMessage<'a>: Deserialize<'a, S>,
    {
        let request = format!("ws://{}:{}", self.addr, self.port)
            .into_client_request()
            .map_err(|_| ThundersClientError::ConnectionFailure)?;
        let (stream, _) = connect_async(request)
            .await
            .map_err(|_| ThundersClientError::ConnectionFailure)?;

        let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel::<InboundAction>();
        let (mut ws_writer, mut ws_receiver) = stream.split();

        let reply_manager = Arc::new(ReplyManager::new());

        tokio::spawn({
            let reply_manager = Arc::clone(&reply_manager);
            async move {
                let mut vacuum_interval = tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    tokio::select! {
                         _ = vacuum_interval.tick() => {
                            reply_manager.vacuum();
                         },
                         Some(inbound_action) = action_rx.recv() => {
                             match inbound_action {
                                 InboundAction::Raw(data) => {
                            if let Err(_) = ws_writer
                                 .send(Message::Binary(data.into()))
                                 .await {
                                     break;
                                }
                             }
                                 InboundAction::Stop => {
                                     break;
                                 }
                             }
                         },
                         Some(Ok(message)) = ws_receiver.next() => {
                            let raw_message = message_into_bytes(message);
                            let raw_message_ref = raw_message.as_slice();
                            if let Ok(output) = <OutputMessage as Deserialize<S>>::deserialize(raw_message_ref) {
                                           match output {
                                                OutputMessage::Connect{correlation_id, success} => {
                                                    if success {
                                                        reply_manager.ok_no_result(correlation_id );
                                                    } else {
                                                        reply_manager.error(correlation_id, ThundersClientError::ConnectionFailure);
                                                    }
                                               },
                                               OutputMessage::Join{correlation_id, success} => {
                                                    if success {
                                                        reply_manager.ok_no_result(correlation_id );
                                                    } else {
                                                        reply_manager.error(correlation_id, ThundersClientError::GameJoinFailure);
                                                    }
                                              },
                                               OutputMessage::Create{correlation_id, success} => {
                                                    if success {
                                                        reply_manager.ok_no_result(correlation_id);
                                                    } else {
                                                        reply_manager.error(correlation_id, ThundersClientError::GameCreationFailure);
                                                    }
                                               }
                                              OutputMessage::Diff{type_, id, finished, data} => {
                                                 if finished {
                                                      if let Ok(room) = active_games.remove(type_.as_ref(), id.as_ref()) {
                                                          room.on_finished();
                                                      }
                                                } else if let Err(err) = active_games.route_message(type_.as_ref(), id.as_ref(), data) {
                                                     log::error!("Message routing failed. Type: {}, Id: {}, Error: {err:?}", type_, id);
                                                  }
                                               }
                                               OutputMessage::GenericError {description} => {
                                                   log::error!("Received error message. Description: {description}");
                                               }
                                        }
                            } else {
                                log::error!("Ignored message due to serialization failure");
                             }
                         },
                    }
                }
            }
        });

        Ok(ClientProtocolHandle {
            action_tx,
            reply_manager,
        })
    }
}
fn message_into_bytes(message: Message) -> Vec<u8> {
    match message {
        Message::Binary(bytes) => bytes.into(),
        Message::Text(bytes) => Bytes::from(bytes).into(),
        _ => {
            vec![]
        }
    }
}
