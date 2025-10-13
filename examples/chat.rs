use std::{
    collections::HashMap,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use thunders::{
    api::schema::json::Json,
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
    if let Err(err) = client_1
        .create::<ChatClient>(
            "lobby_chat",
            "Chat_1".to_string(),
            Default::default(),
            Duration::from_secs(5),
        )
        .await
    {
        panic!("{:?}", err);
    }

    let client_2 = spawn_client(2).await;

    if let Err(err) = client_2
        .join::<ChatClient>("lobby_chat", "Chat_1".to_string(), Duration::from_secs(5))
        .await
    {
        panic!("{:?}", err);
    }

    let mut num_messages = 0;
    loop {
        if let Err(err) = client_1.action::<ChatClient>(
            "lobby_chat",
            "Chat_1",
            ChatAction::IncomingMessage(format!("#{num_messages} message from client_1")),
        ) {
            println!("Exiting due to: {err:?}");
            break;
        }

        if let Err(err) = client_2.action::<ChatClient>(
            "lobby_chat",
            "Chat_1",
            ChatAction::IncomingMessage(format!("#{num_messages} message from client_2")),
        ) {
            println!("Exiting due to: {err:?}");
            break;
        }

        thread::sleep(Duration::from_millis(250));
        num_messages = num_messages + 1;
        if num_messages == (4 * 20) {
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

    if let Err(err) = client.connect(id, Duration::from_secs(5)).await {
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
    type Options = ();

    fn build(_options: &Self::Options) -> Self {
        Self { messages: vec![] }
    }

    fn on_change(&mut self, change: Self::Change) {
        println!("CLIENT received Change: {:?}", change);
    }

    fn on_action(&mut self, action: Self::Action) {
        match action {
            ChatAction::IncomingMessage(message) => {
                self.messages.push(ChatMessage {
                    from: 1,
                    text: message,
                });
            }
        }
    }

    fn on_finish(self) {
        println!("CLIENT received finish signal");
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
                .checked_add(Duration::from_secs(15))
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
