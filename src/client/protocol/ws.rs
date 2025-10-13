use futures::{SinkExt, StreamExt};
use std::sync::{Arc, atomic::AtomicBool};

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
        for<'a> OutputMessage<'a>: Deserialize<S>,
    {
        let request = format!("ws://{}:{}", self.addr, self.port)
            .into_client_request()
            .map_err(|_| ThundersClientError::ConnectionFailure)?;
        let (stream, _) = connect_async(request)
            .await
            .map_err(|_| ThundersClientError::ConnectionFailure)?;

        let (in_action_tx, mut in_action_rx) =
            tokio::sync::mpsc::unbounded_channel::<InboundAction>();
        let (mut ws_writer, mut ws_receiver) = stream.split();

        let running = Arc::new(AtomicBool::new(true));
        let reply_manager = Arc::new(ReplyManager::new(tokio::time::Duration::from_secs(5)));

        tokio::spawn({
            let running = Arc::clone(&running);
            let reply_manager = Arc::clone(&reply_manager);
            async move {
                let running = Arc::clone(&running);

                let mut vacuum_interval = tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    // Add stop in Drop for client
                    tokio::select! {
                         _ = vacuum_interval.tick() => {
                            reply_manager.vacuum();
                         },
                         Some(inbound_action) = in_action_rx.recv() => {
                             match inbound_action {
                                 InboundAction::Raw(data) => {
                            if let Err(_) = ws_writer
                                 .send(Message::Binary(data.into()))
                                 .await {
                                     running.swap(false, std::sync::atomic::Ordering::Release);
                                     break;
                                }
                             }
                                 InboundAction::Stop => {
                                     running.swap(false, std::sync::atomic::Ordering::Release);
                                     break;
                                 }
                             }
                         },
                         Some(Ok(message)) = ws_receiver.next() => {
                            let raw_message = message_into_bytes(message);
                            if let Ok(output) = <OutputMessage as Deserialize<S>>::deserialize(raw_message) {
                                           match output {
                                                OutputMessage::Connect{correlation_id, success} => {
                                                    if success {
                                                        reply_manager.ok(&correlation_id.into_owned(), ());
                                                    }else {
                                                        reply_manager.error(&correlation_id.into_owned(), ThundersClientError::ConnectionFailure);
                                                    }
                                               },
                                               OutputMessage::Join{correlation_id, success} => {
                                                    if success {
                                                        reply_manager.ok(&correlation_id.into_owned(), ());
                                                    }else {
                                                        reply_manager.error(&correlation_id.into_owned(), ThundersClientError::GameJoinFailure);
                                                    }
                                              },
                                               OutputMessage::Create{correlation_id, success} => {
                                                    if success {
                                                        reply_manager.ok(&correlation_id.into_owned(), ());
                                                    }else {
                                                        reply_manager.error(&correlation_id.into_owned(), ThundersClientError::GameCreationFailure);
                                                    }
                                               }
                                              OutputMessage::Diff{type_, id, finished, data} => {


                                                  if finished {
                                                      if let Ok(room) = active_games.remove(type_.as_ref(), id.as_ref()) {
                                                          room.on_finished();
                                                      }
                                                 } else if let Err(err) = active_games.route_message(type_.as_ref(), id.as_ref(), data){
                                                        println!("{:?}", err);
                                                  }
                                               }
                                               OutputMessage::GenericError {description} => {
                                                   println!("CLIENT recevied error. {}", description);
                                               }
                                        }
                            } else {
                                println!("Ignored message");
                             }
                         },

                    }
                }
            }
        });

        Ok(ClientProtocolHandle {
            sender: in_action_tx,
            reply_manager,
            running,
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
