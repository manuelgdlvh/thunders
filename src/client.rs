use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, Bytes, Message, client::IntoClientRequest},
};

use crate::{
    core::hooks::DiffNotification,
    protocol::InputMessage,
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
    fn on_change(&mut self, change: Vec<u8>);
}

impl<S, T> GenericGameState<S> for T
where
    S: Schema,
    T: GameState,
    T::Change: Deserialize<S> + std::fmt::Debug,
{
    fn on_change(&mut self, change: Vec<u8>) {
        if let Ok(change) = <T::Change as Deserialize<S>>::deserialize(change) {
            self.on_change(change);
        } else {
            println!("Ignored message");
        }
    }
}

pub struct ActiveGames<S: Schema> {
    current:
        HashMap<&'static str, RwLock<HashMap<String, Box<dyn GenericGameState<S> + Send + Sync>>>>,
}

impl<S: Schema> ActiveGames<S> {
    pub fn route_message(&self, type_: &str, id: &str, message: Vec<u8>) {
        self.current
            .get(type_)
            .expect("Type should always exists")
            .write()
            .unwrap()
            .get_mut(id)
            .unwrap()
            .as_mut()
            .on_change(message);
    }

    pub fn create<G: GameState + Send + Sync + 'static>(
        &self,
        type_: &'static str,
        id: String,
        game: G,
    ) where
        G::Change: Deserialize<S>,
    {
        self.current
            .get(type_)
            .expect("Type should always exists")
            .write()
            .unwrap()
            .insert(
                id,
                Box::new(game) as Box<dyn GenericGameState<S> + Send + Sync>,
            );
    }
}

pub struct ClientProtocolHandle {
    sender: UnboundedSender<Vec<u8>>,
}

pub trait ClientProtocol {
    fn run<S>(
        self,
        active_games: Arc<ActiveGames<S>>,
    ) -> impl Future<Output = ClientProtocolHandle>
    where
        S: Schema + 'static,
        for<'a> DiffNotification<'a>: Deserialize<S>;
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
    async fn run<S>(self, active_games: Arc<ActiveGames<S>>) -> ClientProtocolHandle
    where
        S: Schema + 'static,
        for<'a> DiffNotification<'a>: Deserialize<S>,
    {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

        let request = format!("ws://{}:{}", self.addr, self.port)
            .into_client_request()
            .unwrap();
        let (stream, _) = connect_async(request).await.unwrap();
        let (mut ws_writer, mut ws_receiver) = stream.split();
        tokio::spawn(async move {
            loop {
                // Add stop in Drop for client
                tokio::select! {
                     Some(raw_action) = rx.recv() => {
                         ws_writer
                             .send(tungstenite::Message::Binary(raw_action.into()))
                             .await
                             .unwrap();
                     },
                     Some(Ok(message)) = ws_receiver.next() => {
                        let raw_message = message_into_bytes(message);
                        if let Ok(notification) = <DiffNotification as Deserialize<S>>::deserialize(raw_message){
                            active_games.route_message(notification.type_.as_ref(), notification.id.as_ref(), notification.data);
                        }else{
                            println!("Ignored message");
                         }
                     },

                }
            }
        });

        ClientProtocolHandle { sender: tx }
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

    pub async fn build(self) -> ThundersClient<S>
    where
        for<'a> DiffNotification<'a>: Deserialize<S>,
    {
        let p_handle = self.protocol.run(Arc::clone(&self.active_games)).await;

        ThundersClient::<S> {
            p_handle,
            active_games: self.active_games,
            connected: false,
        }
    }
}

pub struct ThundersClient<S: Schema> {
    p_handle: ClientProtocolHandle,
    active_games: Arc<ActiveGames<S>>,
    connected: bool,
}

impl<S: Schema + 'static> ThundersClient<S> {
    // Add awaitable callback with timeout to know if successfully created or joined

    pub async fn connect(&mut self, id: u64) {
        self.try_send(InputMessage::Connect { id });
        self.connected = true;
    }

    pub async fn create<G: GameState + Send + Sync + 'static>(
        &self,
        type_: &'static str,
        id: String,
        game: G,
    ) where
        G::Change: Deserialize<S>,
    {
        self.active_games.create(type_, id.clone(), game);
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
        self.active_games.create(type_, id.clone(), game);
        self.try_send(InputMessage::Join {
            type_: type_.to_string(),
            id,
        });
    }

    // Change to references not owned
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
        self.p_handle.sender.send(message.serialize()).unwrap();
    }
}
