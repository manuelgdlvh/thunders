use std::{
    collections::HashMap,
    sync::{Arc, RwLock, atomic::AtomicBool},
    time::{Duration, Instant},
};

use std::sync::atomic::Ordering::Release;

use futures::{SinkExt, StreamExt};
use reply_maybe::ReplyManager;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, Bytes, Message, client::IntoClientRequest},
};

use crate::{
    protocol::{InputMessage, OutputMessage},
    schema::{Deserialize, Schema, Serialize},
};

pub trait GameState {
    type Change: std::fmt::Debug;
    type Action;
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
    current:
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
}

pub struct ClientProtocolHandle {
    sender: UnboundedSender<InboundAction>,
    reply_manager: Arc<ReplyManager<String, (), ThundersClientError>>,
    running: Arc<AtomicBool>,
}

pub trait ClientProtocol {
    fn run<S>(
        self,
        active_games: Arc<ActiveGames<S>>,
    ) -> impl Future<Output = Result<ClientProtocolHandle, ThundersClientError>>
    where
        S: Schema + 'static,
        for<'a> OutputMessage<'a>: Deserialize<S>;
}

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
        let (in_action_tx, mut in_action_rx) =
            tokio::sync::mpsc::unbounded_channel::<InboundAction>();

        let request = format!("ws://{}:{}", self.addr, self.port)
            .into_client_request()
            .map_err(|_| ThundersClientError::ConnectionFailure)?;
        let (stream, _) = connect_async(request)
            .await
            .map_err(|_| ThundersClientError::ConnectionFailure)?;

        let (mut ws_writer, mut ws_receiver) = stream.split();

        let running = Arc::new(AtomicBool::new(true));

        let reply_manager = Arc::new(ReplyManager::<String, (), ThundersClientError>::new(
            tokio::time::Duration::new(1, 0),
        ));
        tokio::spawn({
            let running = Arc::clone(&running);
            let reply_manager = Arc::clone(&reply_manager);
            async move {
                let running = Arc::clone(&running);
                loop {
                    // Add stop in Drop for client
                    tokio::select! {
                         _ = reply_manager.vacuum() => {},
                         Some(inbound_action) = in_action_rx.recv() => {

                             match inbound_action {
                                 InboundAction::Raw(data) => {

                            if let Err(_) = ws_writer
                                 .send(tungstenite::Message::Binary(data.into()))
                                 .await {
                                     running.swap(false, Release);
                                     break;
                                }
                             }

                                 InboundAction::Stop => {
                                     running.swap(false, Release);
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
                                               OutputMessage::Diff{type_, id, finished, data} => {

                                                   if let Err(err) = active_games.route_message(type_.as_ref(), id.as_ref(), data){
                                                        println!("{:?}", err);
                                                   }

                                               }
                                               OutputMessage::GenericError {description} => {
                                                   println!("{}", description);
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

pub enum InboundAction {
    Raw(Vec<u8>),
    Stop,
}

#[derive(Debug)]
pub enum ThundersClientError {
    ConnectionFailure,
    NotRunning,
    RoomNotFound,
    RoomTypeNotFound,
    UnknownMessage,
    NoResponse,
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

// Runtime

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

// Base

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
