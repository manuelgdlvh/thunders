use std::collections::{HashMap, HashSet};

use futures::{SinkExt, StreamExt};
use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, client::IntoClientRequest},
};

pub trait GameState {
    type Change;
    type Action;

    fn on_change(&mut self, change: Self::Change);
}

pub struct ClientProtocolHandle {
    sender: UnboundedSender<Vec<u8>>,
}

pub trait ClientProtocol {
    fn run(self) -> impl Future<Output = ClientProtocolHandle>;
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
    async fn run(self) -> ClientProtocolHandle {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

        let request = "ws://127.0.0.1:8080".into_client_request().unwrap();
        let (stream, _) = connect_async(request).await.unwrap();
        let (mut writer, mut receiver) = stream.split();
        tokio::spawn(async move {
            loop {
                // Add stop in Drop for client
                tokio::select! {
                 Some(raw_action) = rx.recv() => {
                     writer
                         .send(tungstenite::Message::Binary(raw_action.into()))
                         .await
                         .unwrap();
                 },
                 Some(Ok(raw_message)) = receiver.next() => {
                    // Add deserialization of messages and routing for the specific runtime
                    println!("{raw_message}");
                 }
                }
            }
        });

        ClientProtocolHandle { sender: tx }
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

pub struct ThundersClientBuilder<P>
where
    P: ClientProtocol,
{
    protocol: P,
    state_handlers: HashMap<String, String>,
}

impl<P> ThundersClientBuilder<P>
where
    P: ClientProtocol,
{
    pub fn new(protocol: P) -> Self {
        Self {
            protocol,
            state_handlers: HashMap::default(),
        }
    }

    pub fn with_state(mut self, type_: String) -> Self {
        self.state_handlers.insert(type_.clone(), type_);
        self
    }

    pub async fn build(self) -> ThundersClient {
        let p_handle = self.protocol.run().await;

        ThundersClient {
            p_handle,
            registered_types: HashSet::new(),
        }
    }
}

pub struct ThundersClient {
    p_handle: ClientProtocolHandle,
    registered_types: HashSet<&'static str>,
}

impl ThundersClient {
    // Add awaitable callback with timeout to know if successfully created or joined
    pub async fn create(type_: &'static str) {
        todo!()
    }

    pub fn join(type_: &'static str) {
        todo!()
    }

    pub fn action(type_: &'static str) {
        todo!()
    }
}
