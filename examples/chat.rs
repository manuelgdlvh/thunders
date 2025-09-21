use serde::Serialize;
use serde_json::Value;
use std::{collections::HashMap, io::Error, sync::Arc};
use thunders::{
    MultiPlayer,
    core::{
        context::PlayerContext,
        hooks::{Diff, GameHooks},
    },
    protocol::ws::WebSocketProtocol,
    runtime::sync::SyncGameRuntime,
    schema::{DeSerialize, json::Json},
};

#[tokio::main]
pub async fn main() {
    MultiPlayer::new(
        WebSocketProtocol {
            addr: "127.0.0.1:8080",
        },
        Json::default(),
    )
    .register::<SyncGameRuntime, Chat>("chat")
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

impl DeSerialize<Json> for ChatAction {
    fn deserialize(value: Vec<u8>) -> Result<Self, std::io::Error> {
        if let Ok(json) = serde_json::from_slice::<Value>(&value) {
            let text = json.get("text").unwrap().as_str().unwrap().to_string();
            return Ok(ChatAction::IncomingMessage(text));
        }

        Err(Error::new(std::io::ErrorKind::InvalidInput, ""))
    }

    fn serialize(self) -> Vec<u8> {
        unreachable!()
    }
}

// Delta

#[derive(Serialize)]
pub struct ChatMessage {
    from: u64,
    text: String,
}

#[derive(Serialize)]
pub struct ChatDiff {
    messages: Vec<ChatMessage>,
}

impl DeSerialize<Json> for ChatDiff {
    fn deserialize(_value: Vec<u8>) -> Result<Self, std::io::Error> {
        unreachable!()
    }

    fn serialize(self) -> Vec<u8> {
        serde_json::to_string(&self)
            .expect("Delta should always be serializable")
            .into_bytes()
    }
}
