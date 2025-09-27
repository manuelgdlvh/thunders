use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use thunders::{
    MultiPlayer,
    core::{
        context::PlayerContext,
        hooks::{Diff, GameHooks},
    },
    protocol::{ThundersError, ws::WebSocketProtocol},
    runtime::sync::{Settings, SyncRuntime},
    schema::{Deserialize, Serialize, json::Json},
};

#[tokio::main]
pub async fn main() {
    MultiPlayer::new(WebSocketProtocol::new("127.0.0.1", 8080), Json::default())
        .register::<SyncRuntime, Chat>(
            "lobby_chat",
            Settings {
                max_action_await_millis: 2000,
                tick_interval_millis: 100,
            },
        )
        .run()
        .await;
}

pub struct Chat {
    messages: Vec<String>,
}

impl GameHooks for Chat {
    type Delta = ChatDiff;
    type Action = ChatAction;
    type Options = ();

    fn build(_options: Self::Options) -> Self {
        Self { messages: vec![] }
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
                    delta: ChatDiff { messages },
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

    fn is_finished(&self) -> bool {
        false
    }
}

// Action
pub enum ChatAction {
    IncomingMessage(String),
}

impl Deserialize<Json> for ChatAction {
    fn deserialize(value: Vec<u8>) -> Result<Self, ThundersError> {
        const TEXT: &str = "text";

        if let Ok(json) = serde_json::from_slice::<Value>(&value) {
            let text = json
                .get(TEXT)
                .ok_or(ThundersError::DeserializationFailure)?
                .as_str()
                .ok_or(ThundersError::DeserializationFailure)?
                .to_string();
            Ok(ChatAction::IncomingMessage(text))
        } else {
            Err(ThundersError::InvalidInput)
        }
    }
}

#[derive(serde::Serialize)]
pub struct ChatMessage {
    from: u64,
    text: String,
}

#[derive(serde::Serialize)]
pub struct ChatDiff {
    messages: Vec<ChatMessage>,
}

impl Serialize<Json> for ChatDiff {
    fn serialize(self) -> Vec<u8> {
        serde_json::to_string(&self)
            .expect("Should always be serializable")
            .into_bytes()
    }
}
