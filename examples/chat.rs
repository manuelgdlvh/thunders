use std::sync::Arc;

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
        Json {},
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

    fn diff<'a>(
        &self,
        _connected: &'a [Arc<PlayerContext>],
        actions: &[(u64, Self::Action)],
    ) -> Vec<Diff<ChatDiff>> {
        let mut result = Vec::new();

        for (_, action) in actions {
            match action {
                ChatAction::IncomingMessage(text) => {
                    result.push(Diff::Target {
                        ids: vec![2],
                        delta: ChatDiff {
                            messages: vec![text.clone()],
                        },
                    });
                }
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

    fn is_finished(&self) -> bool {
        false
    }
}

// Action
pub enum ChatAction {
    IncomingMessage(String),
}

impl DeSerialize<Json> for ChatAction {
    fn deserialize(_value: Vec<u8>) -> Result<Self, std::io::Error> {
        Ok(Self::IncomingMessage("test".to_string()))
    }

    fn serialize(self) -> Vec<u8> {
        unreachable!()
    }
}

// Delta

pub struct ChatDiff {
    messages: Vec<String>,
}

impl DeSerialize<Json> for ChatDiff {
    fn deserialize(_value: Vec<u8>) -> Result<Self, std::io::Error> {
        unreachable!()
    }

    fn serialize(self) -> Vec<u8> {
        self.messages.join(",").into_bytes()
    }
}
