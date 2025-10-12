use std::{
    collections::HashMap,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use thunders::{
    api::{
        error::ThundersError,
        schema::{Deserialize, Serialize, json::Json},
    },
    client::{ThundersClientBuilder, protocol::ws::WebSocketClientProtocol, state::GameState},
    server::{
        ThundersServer,
        context::PlayerContext,
        hooks::{Diff, GameHooks},
        protocol::ws::WebSocketProtocol,
        runtime::sync::{Settings, SyncRuntime},
    },
};

#[tokio::main]
pub async fn main() {
    tokio::spawn(async move {
        ThundersServer::new(WebSocketProtocol::new("127.0.0.1", 8080), Json::default())
            .register::<SyncRuntime, Chat>(
                "lobby_chat",
                Settings {
                    max_action_await_millis: 2000,
                    tick_interval_millis: 100,
                },
            )
            .run()
            .await
    });

    let client_1 = spawn_client(1).await;
    client_1
        .create(
            "lobby_chat",
            "Chat_1".to_string(),
            ChatClient { messages: vec![] },
        )
        .await;

    let client_2 = spawn_client(2).await;
    client_2.join(
        "lobby_chat",
        "Chat_1".to_string(),
        ChatClient { messages: vec![] },
    );

    let mut num_messages = 0;
    loop {
        client_1.action::<ChatClient>(
            "lobby_chat",
            "Chat_1".to_string(),
            ChatAction::IncomingMessage(format!("#{num_messages} message from client_1")),
        );
        client_2.action::<ChatClient>(
            "lobby_chat",
            "Chat_1".to_string(),
            ChatAction::IncomingMessage(format!("#{num_messages} message from client_2")),
        );

        thread::sleep(Duration::from_millis(250));
        num_messages = num_messages + 1;
        if num_messages == (4 * 60) {
            break;
        }
    }
}

async fn spawn_client(id: u64) -> thunders::client::ThundersClient<Json> {
    let mut client = ThundersClientBuilder::new(
        WebSocketClientProtocol::new("127.0.0.1".to_string(), 8080),
        Json::default(),
    )
    .register("lobby_chat")
    .build()
    .await
    .expect("Should initialize client successfully");

    if let Err(err) = client.connect(id).await {
        panic!("{:?}", err);
    }
    client
}

// Client

pub struct ChatClient {
    messages: Vec<ChatMessage>,
}

impl GameState for ChatClient {
    type Action = ChatAction;
    type Change = ChatDiff;

    fn on_change(&mut self, change: Self::Change) {
        println!("CLIENT received Change: {:?}", change);
    }
}

// Server

pub struct Chat {
    messages: Vec<String>,
    close_at: Instant,
}

impl GameHooks for Chat {
    type Delta = ChatDiff;
    type Action = ChatAction;
    type Options = ();

    fn build(_options: Self::Options) -> Self {
        Self {
            messages: vec![],
            close_at: Instant::now()
                .checked_add(Duration::from_secs(60))
                .expect("Add one minute should not overflow"),
        }
    }

    fn diff(
        &self,
        player_cxts: &HashMap<u64, Arc<PlayerContext>>,
        actions: &[(u64, Self::Action)],
    ) -> Vec<Diff<ChatDiff>> {
        let mut inbox: HashMap<u64, Vec<ChatMessage>> = HashMap::new();
        for (sender_id, action) in actions {
            match action {
                ChatAction::IncomingMessage(text) => {
                    for &rid in player_cxts.keys() {
                        if rid != *sender_id {
                            inbox.entry(rid).or_default().push(ChatMessage {
                                from: *sender_id,
                                text: text.clone(),
                            });
                        }
                    }
                }
            }
        }

        let mut result = Vec::with_capacity(inbox.len());
        for (rid, messages) in inbox {
            if !messages.is_empty() {
                result.push(Diff::Target {
                    ids: vec![rid],
                    delta: ChatDiff::MessagesAdded { messages },
                });
            }
        }

        result
    }

    fn update(&mut self, actions: Vec<(u64, Self::Action)>) {
        for (_, action) in actions {
            match action {
                ChatAction::IncomingMessage(text) => self.messages.push(text),
            }
        }
    }

    fn leave(&self, player_cxt: &PlayerContext) -> Option<Diff<Self::Delta>> {
        None
    }

    fn join(&self, player_cxt: &PlayerContext) -> Option<Vec<Diff<Self::Delta>>> {
        None
    }

    fn finish(&self) -> (bool, Option<Diff<Self::Delta>>) {
        if Instant::now() >= self.close_at {
            (
                true,
                Some(Diff::All {
                    delta: ChatDiff::ChatClosed,
                }),
            )
        } else {
            (false, None)
        }
    }
}

// Action
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ChatAction {
    IncomingMessage(String),
}

impl Deserialize<Json> for ChatAction {
    fn deserialize(value: Vec<u8>) -> Result<Self, ThundersError> {
        if let Ok(serialized) = serde_json::from_slice(&value) {
            Ok(serialized)
        } else {
            Err(ThundersError::DeserializationFailure)
        }
    }
}

impl Serialize<Json> for ChatAction {
    fn serialize(self) -> Vec<u8> {
        serde_json::to_vec(&self).expect("Should always be serializable")
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    from: u64,
    text: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ChatDiff {
    MessagesAdded { messages: Vec<ChatMessage> },
    ChatClosed,
}

// TODO: Autoimplement if implement serde::DeSerialize

impl Serialize<Json> for ChatDiff {
    fn serialize(self) -> Vec<u8> {
        serde_json::to_vec(&self).expect("Should always be serializable")
    }
}

impl Deserialize<Json> for ChatDiff {
    fn deserialize(value: Vec<u8>) -> Result<Self, ThundersError> {
        if let Ok(serialized) = serde_json::from_slice(&value) {
            Ok(serialized)
        } else {
            Err(ThundersError::DeserializationFailure)
        }
    }
}
